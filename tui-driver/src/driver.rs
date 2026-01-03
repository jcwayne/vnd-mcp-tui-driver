//! Core TUI driver implementation

use crate::error::{Result, TuiError};
use crate::keys::Key;
use crate::mouse::{mouse_click, mouse_double_click, MouseButton};
use crate::snapshot::{build_snapshot, render_screenshot, Screenshot, Snapshot};
use crate::terminal::TuiTerminal;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Default capacity for debug buffers (characters)
const BUFFER_CAPACITY: usize = 10000;

/// Ring buffer for storing last N characters of I/O
#[derive(Debug)]
pub struct RingBuffer {
    data: Mutex<VecDeque<char>>,
    capacity: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Push a string to the buffer, evicting old chars if needed
    pub fn push_str(&self, s: &str) {
        let mut data = self.data.lock();
        for c in s.chars() {
            if data.len() >= self.capacity {
                data.pop_front();
            }
            data.push_back(c);
        }
    }

    /// Get the last N characters from the buffer
    pub fn get_last(&self, n: usize) -> String {
        let data = self.data.lock();
        let skip = data.len().saturating_sub(n);
        data.iter().skip(skip).collect()
    }

    /// Get all characters in the buffer
    pub fn get_all(&self) -> String {
        let data = self.data.lock();
        data.iter().collect()
    }

    /// Get the current number of characters in the buffer
    pub fn len(&self) -> usize {
        self.data.lock().len()
    }

    /// Check if the buffer is empty
    pub fn is_empty(&self) -> bool {
        self.data.lock().is_empty()
    }

    /// Clear the buffer
    pub fn clear(&self) {
        self.data.lock().clear();
    }
}

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

    /// Terminal emulator (wezterm-based)
    terminal: Arc<TuiTerminal>,

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

    /// Debug buffer: raw input sent to process (escape sequences)
    input_buffer: Arc<RingBuffer>,

    /// Debug buffer: raw PTY output (escape sequences included)
    output_buffer: Arc<RingBuffer>,
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

        // Initialize terminal with scrollback
        let terminal = Arc::new(TuiTerminal::new(
            options.rows,
            options.cols,
            500, // scrollback lines
        ));
        let last_update = Arc::new(AtomicU64::new(current_timestamp_ms()));
        let running = Arc::new(AtomicBool::new(true));

        // Initialize debug buffers
        let input_buffer = Arc::new(RingBuffer::new(BUFFER_CAPACITY));
        let output_buffer = Arc::new(RingBuffer::new(BUFFER_CAPACITY));

        // Spawn background reader thread (not tokio task - PTY read is blocking)
        let reader_thread = {
            let terminal = Arc::clone(&terminal);
            let last_update = Arc::clone(&last_update);
            let running = Arc::clone(&running);
            let output_buffer = Arc::clone(&output_buffer);

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
                            // Store raw output in debug buffer
                            let text = String::from_utf8_lossy(&buf[..n]);
                            output_buffer.push_str(&text);

                            // Feed bytes to terminal
                            terminal.advance_bytes(&buf[..n]);
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
            terminal,
            last_update,
            running,
            cols: AtomicU16::new(options.cols),
            rows: AtomicU16::new(options.rows),
            _reader_handle: Some(reader_thread),
            input_buffer,
            output_buffer,
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
        self.terminal.with_screen(|screen| {
            let num_rows = screen.physical_rows;
            let num_cols = screen.physical_cols;
            let mut result = String::new();

            for row_idx in 0..num_rows {
                let phys_idx = screen.phys_row(row_idx as i64);
                let mut row_text = String::new();

                screen.for_each_phys_line(|idx, line| {
                    if idx == phys_idx {
                        // Build row text from visible cells
                        let mut last_col = 0;
                        for cell_ref in line.visible_cells() {
                            let col = cell_ref.cell_index();
                            // Fill gaps with spaces
                            while last_col < col && last_col < num_cols {
                                row_text.push(' ');
                                last_col += 1;
                            }
                            if col < num_cols {
                                let text = cell_ref.str();
                                if text.is_empty() {
                                    row_text.push(' ');
                                } else {
                                    row_text.push_str(text);
                                }
                                last_col = col + 1;
                            }
                        }
                        // Fill remaining with spaces
                        while last_col < num_cols {
                            row_text.push(' ');
                            last_col += 1;
                        }
                    }
                });

                result.push_str(row_text.trim_end());
                result.push('\n');
            }

            // Trim trailing empty lines
            while result.ends_with("\n\n") {
                result.pop();
            }

            result
        })
    }

    /// Get accessibility-style snapshot of current screen
    pub fn snapshot(&self) -> Snapshot {
        self.terminal.with_screen(|screen| build_snapshot(screen))
    }

    /// Get a PNG screenshot of the current screen
    pub fn screenshot(&self) -> Screenshot {
        self.terminal.with_screen(|screen| render_screenshot(screen))
    }

    /// Send text to the terminal
    pub fn send_text(&self, text: &str) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        // Record to input buffer
        self.input_buffer.push_str(text);

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

        // Record raw escape sequence to input buffer
        let text = String::from_utf8_lossy(&bytes);
        self.input_buffer.push_str(&text);

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

            // Record raw escape sequence to input buffer
            let text = String::from_utf8_lossy(&bytes);
            self.input_buffer.push_str(&text);

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

        // Update terminal dimensions
        self.terminal.resize(rows, cols);

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

    /// Get last N characters from input buffer (raw escape sequences sent to process)
    pub fn get_input_buffer(&self, n: usize) -> String {
        self.input_buffer.get_last(n)
    }

    /// Get last N characters from output buffer (raw PTY output)
    pub fn get_output_buffer(&self, n: usize) -> String {
        self.output_buffer.get_last(n)
    }

    /// Get scrollback line count (number of lines that have scrolled off screen)
    ///
    /// Use get_output_buffer() for raw output history including scrollback content.
    pub fn get_scrollback(&self) -> usize {
        self.terminal.scrollback()
    }

    /// Clear all debug buffers
    pub fn clear_buffers(&self) {
        self.input_buffer.clear();
        self.output_buffer.clear();
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
