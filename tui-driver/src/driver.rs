//! Core TUI driver implementation

use crate::error::{Result, TuiError};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Configuration for launching a TUI session
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    pub command: String,
    pub args: Vec<String>,
    pub cols: u16,
    pub rows: u16,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            cols: 80,
            rows: 24,
            env: Vec::new(),
            cwd: None,
        }
    }
}

impl LaunchOptions {
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            ..Default::default()
        }
    }

    pub fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }
}

/// Headless TUI driver
pub struct TuiDriver {
    /// Session identifier
    session_id: String,

    /// PTY master handle for writing
    master_writer: Mutex<Box<dyn Write + Send>>,

    /// Child process handle
    child: Mutex<Box<dyn Child + Send + Sync>>,

    /// Terminal parser state
    parser: Arc<Mutex<vt100::Parser>>,

    /// Timestamp of last PTY update (for wait_for_idle)
    last_update: Arc<AtomicU64>,

    /// Whether the session is still running
    running: Arc<AtomicBool>,

    /// Terminal dimensions
    cols: u16,
    rows: u16,

    /// Handle to the background reader thread
    _reader_handle: Option<std::thread::JoinHandle<()>>,
}

impl TuiDriver {
    /// Launch a new TUI session
    pub async fn launch(options: LaunchOptions) -> Result<Self> {
        let session_id = uuid::Uuid::new_v4().to_string();

        // Create PTY
        let pty_system = native_pty_system();
        let pty_pair = pty_system
            .openpty(PtySize {
                rows: options.rows,
                cols: options.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TuiError::PtyError(e.to_string()))?;

        // Build command
        let mut cmd = CommandBuilder::new(&options.command);
        for arg in &options.args {
            cmd.arg(arg);
        }
        cmd.env("TERM", "xterm-256color");
        for (key, value) in &options.env {
            cmd.env(key, value);
        }
        if let Some(cwd) = &options.cwd {
            cmd.cwd(cwd);
        }

        // Spawn child process
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TuiError::LaunchFailed(e.to_string()))?;

        // Get reader and writer
        let master_reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| TuiError::PtyError(e.to_string()))?;
        let master_writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| TuiError::PtyError(e.to_string()))?;

        // Initialize parser
        let parser = Arc::new(Mutex::new(vt100::Parser::new(
            options.rows,
            options.cols,
            0,
        )));
        let last_update = Arc::new(AtomicU64::new(current_timestamp_ms()));
        let running = Arc::new(AtomicBool::new(true));

        // Spawn background reader thread (not tokio task - PTY read is blocking)
        let reader_thread = {
            let parser = Arc::clone(&parser);
            let last_update = Arc::clone(&last_update);
            let running = Arc::clone(&running);

            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let mut master_reader = master_reader;

                while running.load(Ordering::SeqCst) {
                    match master_reader.read(&mut buf) {
                        Ok(0) => {
                            // EOF - process exited
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(n) => {
                            // Feed bytes to parser
                            let mut parser = parser.lock();
                            parser.process(&buf[..n]);
                            last_update.store(current_timestamp_ms(), Ordering::SeqCst);
                        }
                        Err(e) => {
                            if e.kind() != std::io::ErrorKind::WouldBlock {
                                running.store(false, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                }
            })
        };

        Ok(Self {
            session_id,
            master_writer: Mutex::new(master_writer),
            child: Mutex::new(child),
            parser,
            last_update,
            running,
            cols: options.cols,
            rows: options.rows,
            _reader_handle: Some(reader_thread),
        })
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Check if session is still running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get terminal dimensions
    pub fn size(&self) -> (u16, u16) {
        (self.cols, self.rows)
    }

    /// Get plain text snapshot of current screen
    pub fn text(&self) -> String {
        let parser = self.parser.lock();
        let screen = parser.screen();
        let mut result = String::new();

        for row in 0..screen.size().0 {
            let row_text: String = (0..screen.size().1)
                .map(|col| {
                    screen
                        .cell(row, col)
                        .map(|c| c.contents())
                        .unwrap_or_else(|| " ".to_string())
                })
                .collect::<Vec<_>>()
                .join("");
            result.push_str(row_text.trim_end());
            result.push('\n');
        }

        // Trim trailing empty lines
        while result.ends_with("\n\n") {
            result.pop();
        }

        result
    }

    /// Send text to the terminal
    pub fn send_text(&self, text: &str) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let mut writer = self.master_writer.lock();
        writer.write_all(text.as_bytes())?;
        writer.flush()?;
        Ok(())
    }

    /// Wait for screen to settle (no updates for specified duration)
    pub async fn wait_for_idle(&self, idle_ms: u64, timeout_ms: u64) -> Result<()> {
        let start = tokio::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        loop {
            if start.elapsed() > timeout {
                return Err(TuiError::Timeout);
            }

            let last = self.last_update.load(Ordering::SeqCst);
            let now = current_timestamp_ms();

            if now - last >= idle_ms {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Wait for specific text to appear on screen
    pub async fn wait_for_text(&self, text: &str, timeout_ms: u64) -> Result<bool> {
        let start = tokio::time::Instant::now();
        let timeout = Duration::from_millis(timeout_ms);

        loop {
            if self.text().contains(text) {
                return Ok(true);
            }

            if start.elapsed() > timeout {
                return Ok(false);
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Close the session
    pub async fn close(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        // Kill the child process
        let mut child = self.child.lock();
        child.kill().ok();

        Ok(())
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
