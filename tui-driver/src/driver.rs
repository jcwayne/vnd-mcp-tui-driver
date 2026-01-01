//! Core TUI driver implementation

use crate::error::{Result, TuiError};
use crate::keys::Key;
use crate::mouse::{mouse_click, mouse_double_click, MouseButton};
use crate::snapshot::{build_snapshot, render_screenshot, Screenshot, Snapshot};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Information about a TUI session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub command: String,
    pub cols: u16,
    pub rows: u16,
    pub running: bool,
}

/// Signals that can be sent to the process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Int,
    Term,
    Hup,
    Kill,
    Quit,
}

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

    /// Command that was launched
    command: String,

    /// PTY master for resize operations
    master: Mutex<Box<dyn MasterPty + Send>>,

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

    /// Terminal columns (using atomics for resize)
    cols: AtomicU16,
    /// Terminal rows (using atomics for resize)
    rows: AtomicU16,

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
            command: options.command.clone(),
            master: Mutex::new(pty_pair.master),
            master_writer: Mutex::new(master_writer),
            child: Mutex::new(child),
            parser,
            last_update,
            running,
            cols: AtomicU16::new(options.cols),
            rows: AtomicU16::new(options.rows),
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
        (
            self.cols.load(Ordering::SeqCst),
            self.rows.load(Ordering::SeqCst),
        )
    }

    /// Get session information
    pub fn info(&self) -> SessionInfo {
        SessionInfo {
            session_id: self.session_id.clone(),
            command: self.command.clone(),
            cols: self.cols.load(Ordering::SeqCst),
            rows: self.rows.load(Ordering::SeqCst),
            running: self.is_running(),
        }
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

    /// Get accessibility-style snapshot of current screen
    pub fn snapshot(&self) -> Snapshot {
        let parser = self.parser.lock();
        let screen = parser.screen();
        build_snapshot(screen)
    }

    /// Get a PNG screenshot of the current screen
    pub fn screenshot(&self) -> Screenshot {
        let parser = self.parser.lock();
        let screen = parser.screen();
        render_screenshot(screen)
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

    /// Send a single key to the terminal
    pub fn press_key(&self, key: &Key) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let bytes = key.to_escape_sequence();
        let mut writer = self.master_writer.lock();
        writer.write_all(&bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Send multiple keys to the terminal
    pub fn press_keys(&self, keys: &[Key]) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let mut writer = self.master_writer.lock();
        for key in keys {
            let bytes = key.to_escape_sequence();
            writer.write_all(&bytes)?;
        }
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

    /// Resize the terminal
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let master = self.master.lock();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TuiError::ResizeFailed(e.to_string()))?;

        // Update stored dimensions
        self.cols.store(cols, Ordering::SeqCst);
        self.rows.store(rows, Ordering::SeqCst);

        // Update parser dimensions
        {
            let mut parser = self.parser.lock();
            parser.set_size(rows, cols);
        }

        Ok(())
    }

    /// Send a signal to the child process
    pub fn send_signal(&self, signal: Signal) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        match signal {
            Signal::Int => {
                // Send Ctrl+C (0x03)
                self.send_text("\x03")?;
            }
            Signal::Kill | Signal::Term | Signal::Hup | Signal::Quit => {
                let mut child = self.child.lock();
                child
                    .kill()
                    .map_err(|e| TuiError::SignalFailed(e.to_string()))?;
            }
        }
        Ok(())
    }

    /// Validate that coordinates are within terminal bounds.
    ///
    /// Coordinates must be 1-based and within the terminal dimensions.
    /// Returns an error if x=0, y=0, x>cols, or y>rows.
    fn validate_coordinates(&self, x: u16, y: u16) -> Result<()> {
        let cols = self.cols.load(Ordering::SeqCst);
        let rows = self.rows.load(Ordering::SeqCst);
        if x == 0 || y == 0 || x > cols || y > rows {
            return Err(TuiError::InvalidCoordinates { x, y });
        }
        Ok(())
    }

    /// Send mouse event bytes to the terminal.
    fn send_mouse_event(&self, bytes: &[u8]) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let mut writer = self.master_writer.lock();
        writer.write_all(bytes)?;
        writer.flush()?;
        Ok(())
    }

    /// Click at the specified coordinates.
    ///
    /// Coordinates are 1-based (x=column, y=row).
    /// Returns an error if coordinates are out of bounds.
    pub fn click_at(&self, x: u16, y: u16) -> Result<()> {
        self.validate_coordinates(x, y)?;
        let bytes = mouse_click(MouseButton::Left, x, y);
        self.send_mouse_event(&bytes)
    }

    /// Click on an element by reference ID.
    ///
    /// Uses the current snapshot to find the element's coordinates.
    /// Returns an error if the reference is not found.
    pub fn click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        let span = snapshot
            .get_by_ref(ref_id)
            .ok_or_else(|| TuiError::RefNotFound(ref_id.to_string()))?;
        self.click_at(span.x, span.y)
    }

    /// Double-click at the specified coordinates.
    ///
    /// Coordinates are 1-based (x=column, y=row).
    /// Returns an error if coordinates are out of bounds.
    pub fn double_click_at(&self, x: u16, y: u16) -> Result<()> {
        self.validate_coordinates(x, y)?;
        let bytes = mouse_double_click(MouseButton::Left, x, y);
        self.send_mouse_event(&bytes)
    }

    /// Double-click on an element by reference ID.
    ///
    /// Uses the current snapshot to find the element's coordinates.
    /// Returns an error if the reference is not found.
    pub fn double_click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        let span = snapshot
            .get_by_ref(ref_id)
            .ok_or_else(|| TuiError::RefNotFound(ref_id.to_string()))?;
        self.double_click_at(span.x, span.y)
    }

    /// Right-click at the specified coordinates.
    ///
    /// Coordinates are 1-based (x=column, y=row).
    /// Returns an error if coordinates are out of bounds.
    pub fn right_click_at(&self, x: u16, y: u16) -> Result<()> {
        self.validate_coordinates(x, y)?;
        let bytes = mouse_click(MouseButton::Right, x, y);
        self.send_mouse_event(&bytes)
    }

    /// Right-click on an element by reference ID.
    ///
    /// Uses the current snapshot to find the element's coordinates.
    /// Returns an error if the reference is not found.
    pub fn right_click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        let span = snapshot
            .get_by_ref(ref_id)
            .ok_or_else(|| TuiError::RefNotFound(ref_id.to_string()))?;
        self.right_click_at(span.x, span.y)
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
