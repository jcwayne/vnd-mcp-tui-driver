# Milestone 6: Production Ready - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add remaining API features and polish for production use.

**Architecture:** Complete TuiDriver API + MCP tools + updated docs.

**Tech Stack:** Rust, nix for signals, portable-pty for resize.

---

## Context

Need to add:
- `resize(cols, rows)` - change terminal dimensions
- `send_signal(signal)` - send signal to process (SIGINT, SIGTERM, etc.)
- `list_sessions()` - list all active sessions
- `get_session(id)` - get session info
- Update README with all tools

Multi-session is already supported via SessionManager.

---

### Task 1: Add resize Method to TuiDriver

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Store PTY master for resize**

The portable-pty crate supports resize. We need to store the master pair.

Add field to TuiDriver:
```rust
/// PTY master for resize operations
master: Mutex<Box<dyn portable_pty::MasterPty + Send>>,
```

Update `launch()` to store the master.

Add resize method:
```rust
/// Resize the terminal
pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
    if !self.is_running() {
        return Err(TuiError::SessionClosed);
    }

    let master = self.master.lock();
    master.resize(PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }).map_err(|e| TuiError::ResizeFailed(e.to_string()))?;

    // Update stored dimensions
    // (need to make cols/rows mutable or use atomics)

    Ok(())
}
```

**Step 2: Add ResizeFailed error**

In error.rs:
```rust
#[error("Resize failed: {0}")]
ResizeFailed(String),
```

**Step 3: Build and test**

Run: `source ~/.cargo/env && cargo build`

**Step 4: Commit**

```bash
git add tui-driver/src/driver.rs tui-driver/src/error.rs
git commit -m "feat(tui-driver): add resize method"
```

---

### Task 2: Add send_signal Method

**Files:**
- Modify: `tui-driver/src/driver.rs`
- Modify: `tui-driver/src/error.rs`

**Step 1: Add Signal enum**

```rust
/// Signals that can be sent to the process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    SIGINT,
    SIGTERM,
    SIGHUP,
    SIGKILL,
    SIGQUIT,
}

impl Signal {
    pub fn as_str(&self) -> &'static str {
        match self {
            Signal::SIGINT => "SIGINT",
            Signal::SIGTERM => "SIGTERM",
            Signal::SIGHUP => "SIGHUP",
            Signal::SIGKILL => "SIGKILL",
            Signal::SIGQUIT => "SIGQUIT",
        }
    }
}
```

**Step 2: Add send_signal method**

For portable-pty, we can use the child's kill() method. For SIGINT specifically, we can send Ctrl+C.

```rust
/// Send a signal to the child process
pub fn send_signal(&self, signal: Signal) -> Result<()> {
    if !self.is_running() {
        return Err(TuiError::SessionClosed);
    }

    match signal {
        Signal::SIGINT => {
            // Send Ctrl+C
            self.send_text("\x03")?;
        }
        Signal::SIGKILL => {
            let mut child = self.child.lock();
            child.kill().map_err(|e| TuiError::SignalFailed(e.to_string()))?;
        }
        _ => {
            // For other signals, try kill
            let mut child = self.child.lock();
            child.kill().map_err(|e| TuiError::SignalFailed(e.to_string()))?;
        }
    }

    Ok(())
}
```

**Step 3: Add SignalFailed error**

```rust
#[error("Failed to send signal: {0}")]
SignalFailed(String),
```

**Step 4: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 5: Commit**

```bash
git add tui-driver/src/driver.rs tui-driver/src/error.rs
git commit -m "feat(tui-driver): add send_signal method"
```

---

### Task 3: Add Session Info Methods

**Files:**
- Modify: `tui-driver/src/driver.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Add SessionInfo struct**

```rust
/// Information about a TUI session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub command: String,
    pub cols: u16,
    pub rows: u16,
    pub running: bool,
}
```

**Step 2: Add info method to TuiDriver**

```rust
/// Get session information
pub fn info(&self) -> SessionInfo {
    SessionInfo {
        session_id: self.session_id.clone(),
        command: self.command.clone(),
        cols: self.cols,
        rows: self.rows,
        running: self.is_running(),
    }
}
```

Need to store command in TuiDriver.

**Step 3: Export from lib.rs**

```rust
pub use driver::SessionInfo;
```

**Step 4: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 5: Commit**

```bash
git add tui-driver/src/driver.rs tui-driver/src/lib.rs
git commit -m "feat(tui-driver): add SessionInfo and info() method"
```

---

### Task 4: Add MCP Tools for New Features

**Files:**
- Modify: `mcp-server/src/tools.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Add parameter types**

```rust
#[derive(Debug, Deserialize)]
pub struct ResizeParams {
    pub session_id: String,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Deserialize)]
pub struct SignalParams {
    pub session_id: String,
    pub signal: String,
}
```

**Step 2: Add tools**

- tui_resize: session_id, cols, rows
- tui_send_signal: session_id, signal
- tui_list_sessions: (no params)
- tui_get_session: session_id

**Step 3: Implement handlers**

For list_sessions, iterate over sessions map.
For get_session, return info().

**Step 4: Build and test**

Run: `source ~/.cargo/env && cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add mcp-server/src/tools.rs mcp-server/src/main.rs
git commit -m "feat(mcp-server): add resize, signal, and session info tools"
```

---

### Task 5: Update README

**Files:**
- Modify: `README.md`

**Step 1: Update with all tools**

List all 19 tools with descriptions:
- Session: tui_launch, tui_close, tui_list_sessions, tui_get_session
- View: tui_text, tui_snapshot, tui_screenshot
- Input: tui_press_key, tui_press_keys, tui_send_text
- Wait: tui_wait_for_text, tui_wait_for_idle
- Mouse: tui_click, tui_click_at, tui_double_click, tui_right_click
- Control: tui_resize, tui_send_signal
- Scripting: tui_run_code

**Step 2: Add usage examples**

**Step 3: Commit**

```bash
git add README.md
git commit -m "docs: update README with all tools and examples"
```

---

### Task 6: Final Verification

**Step 1: Run all checks**

```bash
source ~/.cargo/env
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cargo build --release
```

**Step 2: Verify tool count**

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | cargo run --bin mcp-tui-driver 2>/dev/null | jq '.result.tools | length'
```

Should return 19+ tools.
