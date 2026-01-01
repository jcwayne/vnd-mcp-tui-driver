# Milestone 1: The Viewer - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create a working MCP server that can launch a TUI application, capture text snapshots, and close sessions.

**Architecture:** Two-crate workspace - `tui-driver` (core library) handles PTY and terminal emulation, `mcp-server` (binary) exposes MCP tools over stdio. A background tokio task reads from PTY and feeds vt100::Parser.

**Tech Stack:** Rust 2021, tokio, portable-pty, vt100, rmcp, serde

---

## Task 1: Workspace Setup

**Files:**
- Create: `Cargo.toml`
- Create: `tui-driver/Cargo.toml`
- Create: `tui-driver/src/lib.rs`
- Create: `mcp-server/Cargo.toml`
- Create: `mcp-server/src/main.rs`

**Step 1: Create root workspace Cargo.toml**

```toml
[workspace]
members = ["tui-driver", "mcp-server"]
resolver = "2"
```

**Step 2: Create tui-driver crate**

```bash
mkdir -p tui-driver/src
```

Create `tui-driver/Cargo.toml`:

```toml
[package]
name = "tui-driver"
version = "0.1.0"
edition = "2021"

[dependencies]
portable-pty = "0.8"
vt100 = "0.15"
tokio = { version = "1", features = ["full", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
parking_lot = "0.12"

[dev-dependencies]
tokio-test = "0.4"
```

Create `tui-driver/src/lib.rs`:

```rust
//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;

pub use driver::TuiDriver;
pub use error::TuiError;
```

**Step 3: Create mcp-server crate**

```bash
mkdir -p mcp-server/src
```

Create `mcp-server/Cargo.toml`:

```toml
[package]
name = "mcp-tui-driver"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mcp-tui-driver"
path = "src/main.rs"

[dependencies]
tui-driver = { path = "../tui-driver" }
rmcp = { version = "0.1", features = ["server", "transport-io"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

Create `mcp-server/src/main.rs`:

```rust
//! MCP TUI Driver - MCP server for TUI automation

fn main() {
    println!("mcp-tui-driver placeholder");
}
```

**Step 4: Verify workspace builds**

Run: `cargo build`
Expected: Successful compilation with no errors

**Step 5: Commit**

```bash
git add Cargo.toml tui-driver/ mcp-server/
git commit -m "feat: initialize cargo workspace with tui-driver and mcp-server crates"
```

---

## Task 2: Error Types

**Files:**
- Create: `tui-driver/src/error.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Create error types**

Create `tui-driver/src/error.rs`:

```rust
//! Error types for tui-driver

use thiserror::Error;

/// Errors that can occur in tui-driver operations
#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Failed to launch process: {0}")]
    LaunchFailed(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already closed")]
    SessionClosed,

    #[error("Timeout waiting for condition")]
    Timeout,

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid coordinates: ({x}, {y})")]
    InvalidCoordinates { x: u16, y: u16 },

    #[error("PTY error: {0}")]
    PtyError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type alias for tui-driver operations
pub type Result<T> = std::result::Result<T, TuiError>;
```

**Step 2: Verify it compiles**

Run: `cargo build -p tui-driver`
Expected: Successful compilation

**Step 3: Commit**

```bash
git add tui-driver/src/error.rs tui-driver/src/lib.rs
git commit -m "feat(tui-driver): add error types"
```

---

## Task 3: TuiDriver Core Structure

**Files:**
- Create: `tui-driver/src/driver.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Create driver module with basic structure**

Create `tui-driver/src/driver.rs`:

```rust
//! Core TUI driver implementation

use crate::error::{Result, TuiError};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

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

    /// Handle to stop the background reader task
    _reader_handle: tokio::task::JoinHandle<()>,
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
        let parser = Arc::new(Mutex::new(vt100::Parser::new(options.rows, options.cols, 0)));
        let last_update = Arc::new(AtomicU64::new(0));
        let running = Arc::new(AtomicBool::new(true));

        // Spawn background reader task
        let reader_handle = {
            let parser = Arc::clone(&parser);
            let last_update = Arc::clone(&last_update);
            let running = Arc::clone(&running);

            tokio::spawn(async move {
                Self::reader_task(master_reader, parser, last_update, running).await;
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
            _reader_handle: reader_handle,
        })
    }

    /// Background task that reads from PTY and updates parser
    async fn reader_task(
        mut reader: Box<dyn Read + Send>,
        parser: Arc<Mutex<vt100::Parser>>,
        last_update: Arc<AtomicU64>,
        running: Arc<AtomicBool>,
    ) {
        let mut buf = [0u8; 4096];

        loop {
            // Use blocking read in a spawn_blocking context
            let read_result = tokio::task::spawn_blocking({
                let mut reader = unsafe {
                    // This is safe because we're the only task reading
                    std::ptr::read(&reader as *const _)
                };
                move || reader.read(&mut buf)
            });

            // Reconstruct reader for next iteration
            // Note: This is a workaround; in production we'd use a different approach

            match tokio::time::timeout(Duration::from_millis(100), async {
                // Actually we need a different approach - use channels
            })
            .await
            {
                Ok(_) => {}
                Err(_) => {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }

            // For now, just check if still running
            if !running.load(Ordering::SeqCst) {
                break;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
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
                        .unwrap_or(" ")
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

    /// Close the session
    pub async fn close(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        // Kill the child process
        let mut child = self.child.lock();
        child.kill().ok();

        Ok(())
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p tui-driver`
Expected: Successful compilation (warnings are OK for now)

**Step 3: Commit**

```bash
git add tui-driver/src/driver.rs tui-driver/src/lib.rs
git commit -m "feat(tui-driver): add TuiDriver core structure with launch, text, close"
```

---

## Task 4: Fix Background Reader with Proper Channel-Based I/O

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Rewrite reader task with proper async I/O**

The previous implementation had issues with blocking I/O. Replace the reader task and related code in `tui-driver/src/driver.rs`:

```rust
//! Core TUI driver implementation

use crate::error::{Result, TuiError};
use parking_lot::Mutex;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use tokio::time::{Duration, Instant};

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

    /// Handle to the reader thread
    _reader_thread: Option<thread::JoinHandle<()>>,
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
        let mut master_reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| TuiError::PtyError(e.to_string()))?;
        let master_writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| TuiError::PtyError(e.to_string()))?;

        // Initialize parser
        let parser = Arc::new(Mutex::new(vt100::Parser::new(options.rows, options.cols, 0)));
        let last_update = Arc::new(AtomicU64::new(current_timestamp_ms()));
        let running = Arc::new(AtomicBool::new(true));

        // Spawn background reader thread (not tokio task - PTY read is blocking)
        let reader_thread = {
            let parser = Arc::clone(&parser);
            let last_update = Arc::clone(&last_update);
            let running = Arc::clone(&running);

            thread::spawn(move || {
                let mut buf = [0u8; 4096];

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
                            // Check if it's a "would block" or actual error
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
            _reader_thread: Some(reader_thread),
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
                        .unwrap_or(" ")
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
        writer
            .write_all(text.as_bytes())
            .map_err(|e| TuiError::IoError(e))?;
        writer.flush().map_err(|e| TuiError::IoError(e))?;
        Ok(())
    }

    /// Wait for screen to settle (no updates for specified duration)
    pub async fn wait_for_idle(&self, idle_ms: u64, timeout_ms: u64) -> Result<()> {
        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let idle_duration = idle_ms;

        loop {
            if start.elapsed() > timeout {
                return Err(TuiError::Timeout);
            }

            let last = self.last_update.load(Ordering::SeqCst);
            let now = current_timestamp_ms();

            if now - last >= idle_duration {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Wait for specific text to appear on screen
    pub async fn wait_for_text(&self, text: &str, timeout_ms: u64) -> Result<bool> {
        let start = Instant::now();
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
```

**Step 2: Verify it compiles**

Run: `cargo build -p tui-driver`
Expected: Successful compilation

**Step 3: Commit**

```bash
git add tui-driver/src/driver.rs
git commit -m "fix(tui-driver): use thread for blocking PTY read instead of tokio task"
```

---

## Task 5: Add Integration Test for TuiDriver

**Files:**
- Create: `tui-driver/tests/integration_test.rs`

**Step 1: Write integration test**

Create `tui-driver/tests/integration_test.rs`:

```rust
//! Integration tests for TuiDriver

use tui_driver::{driver::LaunchOptions, TuiDriver};

#[tokio::test]
async fn test_launch_and_text_snapshot() {
    // Launch a simple command that outputs known text
    let options = LaunchOptions::new("echo").args(vec!["Hello, TUI!".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for output to be processed
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let text = driver.text();
    assert!(
        text.contains("Hello, TUI!"),
        "Expected 'Hello, TUI!' in output, got: {:?}",
        text
    );

    driver.close().await.expect("Failed to close");
}

#[tokio::test]
async fn test_launch_interactive_command() {
    // Launch bash and send a command
    let options = LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Send a command
    driver.send_text("echo TEST_OUTPUT\n").expect("Failed to send text");

    // Wait for output
    let found = driver
        .wait_for_text("TEST_OUTPUT", 2000)
        .await
        .expect("Wait failed");

    assert!(found, "Expected to find TEST_OUTPUT in screen");

    // Clean exit
    driver.send_text("exit\n").expect("Failed to send exit");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    driver.close().await.expect("Failed to close");
}

#[tokio::test]
async fn test_wait_for_idle() {
    let options = LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for initial output to settle
    driver
        .wait_for_idle(100, 5000)
        .await
        .expect("Wait for idle failed");

    // Screen should be stable now
    let text1 = driver.text();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let text2 = driver.text();

    assert_eq!(text1, text2, "Screen should be stable after wait_for_idle");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}
```

**Step 2: Run tests**

Run: `cargo test -p tui-driver`
Expected: All tests pass

**Step 3: Commit**

```bash
git add tui-driver/tests/integration_test.rs
git commit -m "test(tui-driver): add integration tests for launch, text, wait_for_idle"
```

---

## Task 6: Basic MCP Server Setup

**Files:**
- Modify: `mcp-server/Cargo.toml`
- Modify: `mcp-server/src/main.rs`
- Create: `mcp-server/src/tools.rs`

**Step 1: Update dependencies in mcp-server/Cargo.toml**

Note: rmcp may need version adjustment based on actual crates.io availability.

```toml
[package]
name = "mcp-tui-driver"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "mcp-tui-driver"
path = "src/main.rs"

[dependencies]
tui-driver = { path = "../tui-driver" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full", "io-std"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
async-trait = "0.1"
```

**Step 2: Create tools module**

Create `mcp-server/src/tools.rs`:

```rust
//! MCP tool definitions for TUI automation

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchParams {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchResult {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionParams {
    pub session_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextResult {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CloseResult {
    pub success: bool,
}
```

**Step 3: Create main MCP server**

Replace `mcp-server/src/main.rs`:

```rust
//! MCP TUI Driver - MCP server for TUI automation

mod tools;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tui_driver::{driver::LaunchOptions, TuiDriver};

use tools::*;

/// Session manager
struct SessionManager {
    sessions: HashMap<String, TuiDriver>,
}

impl SessionManager {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    fn insert(&mut self, driver: TuiDriver) -> String {
        let id = driver.session_id().to_string();
        self.sessions.insert(id.clone(), driver);
        id
    }

    fn get(&self, id: &str) -> Option<&TuiDriver> {
        self.sessions.get(id)
    }

    fn remove(&mut self, id: &str) -> Option<TuiDriver> {
        self.sessions.remove(id)
    }
}

/// JSON-RPC request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// MCP tool definition
#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "tui_launch".to_string(),
            description: "Launch a new TUI session".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Command to run"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Command arguments"
                    },
                    "cols": {
                        "type": "integer",
                        "description": "Terminal width (default: 80)"
                    },
                    "rows": {
                        "type": "integer",
                        "description": "Terminal height (default: 24)"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "tui_text".to_string(),
            description: "Get plain text snapshot of terminal".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID from tui_launch"
                    }
                },
                "required": ["session_id"]
            }),
        },
        ToolDefinition {
            name: "tui_close".to_string(),
            description: "Close a TUI session".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "Session ID to close"
                    }
                },
                "required": ["session_id"]
            }),
        },
    ]
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("mcp-tui-driver starting");

    let sessions = Arc::new(Mutex::new(SessionManager::new()));

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        tracing::debug!("Received: {}", line);

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = JsonRpcResponse::error(Value::Null, -32700, format!("Parse error: {}", e));
                writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
                stdout.flush()?;
                continue;
            }
        };

        let id = request.id.clone().unwrap_or(Value::Null);
        let response = handle_request(&request, Arc::clone(&sessions)).await;

        let response = match response {
            Ok(result) => JsonRpcResponse::success(id, result),
            Err(e) => JsonRpcResponse::error(id, -32000, e.to_string()),
        };

        let response_str = serde_json::to_string(&response)?;
        tracing::debug!("Sending: {}", response_str);
        writeln!(stdout, "{}", response_str)?;
        stdout.flush()?;
    }

    Ok(())
}

async fn handle_request(
    request: &JsonRpcRequest,
    sessions: Arc<Mutex<SessionManager>>,
) -> Result<Value> {
    match request.method.as_str() {
        // MCP initialization
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "mcp-tui-driver",
                "version": "0.1.0"
            }
        })),

        // List available tools
        "tools/list" => Ok(json!({
            "tools": get_tool_definitions()
        })),

        // Call a tool
        "tools/call" => {
            let tool_name = request.params["name"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;
            let arguments = &request.params["arguments"];

            match tool_name {
                "tui_launch" => {
                    let params: LaunchParams = serde_json::from_value(arguments.clone())?;
                    let options = LaunchOptions::new(&params.command)
                        .args(params.args)
                        .size(params.cols, params.rows);

                    let driver = TuiDriver::launch(options).await?;
                    let session_id = {
                        let mut mgr = sessions.lock().await;
                        mgr.insert(driver)
                    };

                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Session launched: {}", session_id)
                        }],
                        "session_id": session_id
                    }))
                }

                "tui_text" => {
                    let params: SessionParams = serde_json::from_value(arguments.clone())?;
                    let mgr = sessions.lock().await;
                    let driver = mgr
                        .get(&params.session_id)
                        .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

                    let text = driver.text();

                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": text
                        }]
                    }))
                }

                "tui_close" => {
                    let params: SessionParams = serde_json::from_value(arguments.clone())?;
                    let mut mgr = sessions.lock().await;
                    if let Some(driver) = mgr.remove(&params.session_id) {
                        driver.close().await?;
                    }

                    Ok(json!({
                        "content": [{
                            "type": "text",
                            "text": "Session closed"
                        }]
                    }))
                }

                _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
            }
        }

        // Notifications (no response needed for some)
        "notifications/initialized" => Ok(json!({})),

        _ => Err(anyhow::anyhow!("Unknown method: {}", request.method)),
    }
}
```

**Step 4: Verify it compiles**

Run: `cargo build -p mcp-tui-driver`
Expected: Successful compilation

**Step 5: Commit**

```bash
git add mcp-server/
git commit -m "feat(mcp-server): implement basic MCP server with tui_launch, tui_text, tui_close"
```

---

## Task 7: Manual MCP Server Test

**Files:** None (manual testing)

**Step 1: Build release binary**

Run: `cargo build --release`

**Step 2: Test MCP handshake**

Run:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/mcp-tui-driver
```

Expected output containing:
```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05",...}}
```

**Step 3: Test tools/list**

Run:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | ./target/release/mcp-tui-driver
```

Expected: List of tools including tui_launch, tui_text, tui_close

**Step 4: Document in README**

Create or update `README.md`:

```markdown
# mcp-tui-driver

MCP server for headless TUI automation.

## Quick Test

```bash
cargo build --release

# Test MCP handshake
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/mcp-tui-driver
```

## Available Tools

- `tui_launch` - Launch a TUI session
- `tui_text` - Get text snapshot
- `tui_close` - Close a session
```

**Step 5: Commit**

```bash
git add README.md
git commit -m "docs: add basic README with quick test instructions"
```

---

## Task 8: Final Verification

**Step 1: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No errors (warnings as errors)

**Step 3: Format check**

Run: `cargo fmt --check`
Expected: No formatting issues

**Step 4: Fix any issues found**

If clippy or fmt report issues, fix them and commit:

```bash
cargo fmt
git add -A
git commit -m "style: apply rustfmt and fix clippy warnings"
```

**Step 5: Final commit summary**

Run: `git log --oneline`

Expected commits:
1. feat: initialize cargo workspace with tui-driver and mcp-server crates
2. feat(tui-driver): add error types
3. feat(tui-driver): add TuiDriver core structure with launch, text, close
4. fix(tui-driver): use thread for blocking PTY read instead of tokio task
5. test(tui-driver): add integration tests for launch, text, wait_for_idle
6. feat(mcp-server): implement basic MCP server with tui_launch, tui_text, tui_close
7. docs: add basic README with quick test instructions

---

## Milestone 1 Complete

After completing all tasks, you will have:

- A working `tui-driver` library that can:
  - Launch terminal applications
  - Capture text snapshots
  - Wait for idle/text
  - Close sessions

- A working `mcp-tui-driver` MCP server exposing:
  - `tui_launch` - Start sessions
  - `tui_text` - Get text content
  - `tui_close` - End sessions

Ready to proceed to Milestone 2: The Keyboard.
