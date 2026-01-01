# Milestone 4: The Clicker - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add mouse support with SGR mouse mode sequences and click operations.

**Architecture:** Mouse module for SGR sequences + TuiDriver methods + MCP tools.

**Tech Stack:** Rust, SGR 1006 mouse protocol.

---

## Context

Terminals support mouse events via escape sequences. SGR 1006 format is widely supported:
- Press: `\x1b[<0;X;YM` (button 0=left, 1=middle, 2=right)
- Release: `\x1b[<0;X;Ym`
- Coordinates are 1-based

We'll implement:
- `click(ref)` - click on element by ref
- `click_at(x, y)` - click at coordinates
- `double_click(ref)` - double click
- `right_click(ref)` - right click

---

### Task 1: Create Mouse Module

**Files:**
- Create: `tui-driver/src/mouse.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Create mouse.rs**

```rust
//! Mouse event generation for terminal

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

impl MouseButton {
    fn code(&self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        }
    }
}

/// Generate SGR mouse click sequence (press + release)
pub fn mouse_click(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    let mut seq = Vec::new();
    // Press
    seq.extend(format!("\x1b[<{};{};{}M", button.code(), x, y).as_bytes());
    // Release
    seq.extend(format!("\x1b[<{};{};{}m", button.code(), x, y).as_bytes());
    seq
}

/// Generate SGR double-click sequence
pub fn mouse_double_click(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    let mut seq = Vec::new();
    seq.extend(mouse_click(button, x, y));
    seq.extend(mouse_click(button, x, y));
    seq
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_click_left() {
        let seq = mouse_click(MouseButton::Left, 10, 5);
        let expected = "\x1b[<0;10;5M\x1b[<0;10;5m";
        assert_eq!(String::from_utf8_lossy(&seq), expected);
    }

    #[test]
    fn test_mouse_click_right() {
        let seq = mouse_click(MouseButton::Right, 1, 1);
        let expected = "\x1b[<2;1;1M\x1b[<2;1;1m";
        assert_eq!(String::from_utf8_lossy(&seq), expected);
    }

    #[test]
    fn test_double_click() {
        let seq = mouse_double_click(MouseButton::Left, 5, 3);
        // Should be two click sequences
        assert!(seq.len() > mouse_click(MouseButton::Left, 5, 3).len());
    }
}
```

**Step 2: Export from lib.rs**

```rust
pub mod mouse;
pub use mouse::MouseButton;
```

**Step 3: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 4: Commit**

```bash
git add tui-driver/src/mouse.rs tui-driver/src/lib.rs
git commit -m "feat(tui-driver): add mouse event generation module"
```

---

### Task 2: Add Mouse Methods to TuiDriver

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Add methods**

```rust
use crate::mouse::{mouse_click, mouse_double_click, MouseButton};

impl TuiDriver {
    /// Click at specific coordinates (1-based)
    pub fn click_at(&self, x: u16, y: u16) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }
        if x == 0 || y == 0 || x > self.cols || y > self.rows {
            return Err(TuiError::InvalidCoordinates { x, y });
        }

        let seq = mouse_click(MouseButton::Left, x, y);
        let mut writer = self.master_writer.lock();
        writer.write_all(&seq)?;
        writer.flush()?;
        Ok(())
    }

    /// Click on element by reference ID
    pub fn click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        match snapshot.get_by_ref(ref_id) {
            Some(span) => self.click_at(span.x, span.y),
            None => Err(TuiError::RefNotFound(ref_id.to_string())),
        }
    }

    /// Double-click at coordinates
    pub fn double_click_at(&self, x: u16, y: u16) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }
        if x == 0 || y == 0 || x > self.cols || y > self.rows {
            return Err(TuiError::InvalidCoordinates { x, y });
        }

        let seq = mouse_double_click(MouseButton::Left, x, y);
        let mut writer = self.master_writer.lock();
        writer.write_all(&seq)?;
        writer.flush()?;
        Ok(())
    }

    /// Double-click on element by reference ID
    pub fn double_click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        match snapshot.get_by_ref(ref_id) {
            Some(span) => self.double_click_at(span.x, span.y),
            None => Err(TuiError::RefNotFound(ref_id.to_string())),
        }
    }

    /// Right-click at coordinates
    pub fn right_click_at(&self, x: u16, y: u16) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }
        if x == 0 || y == 0 || x > self.cols || y > self.rows {
            return Err(TuiError::InvalidCoordinates { x, y });
        }

        let seq = mouse_click(MouseButton::Right, x, y);
        let mut writer = self.master_writer.lock();
        writer.write_all(&seq)?;
        writer.flush()?;
        Ok(())
    }

    /// Right-click on element by reference ID
    pub fn right_click(&self, ref_id: &str) -> Result<()> {
        let snapshot = self.snapshot();
        match snapshot.get_by_ref(ref_id) {
            Some(span) => self.right_click_at(span.x, span.y),
            None => Err(TuiError::RefNotFound(ref_id.to_string())),
        }
    }
}
```

**Step 2: Add RefNotFound error variant to error.rs**

```rust
#[error("Element reference not found: {0}")]
RefNotFound(String),
```

**Step 3: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 4: Commit**

```bash
git add tui-driver/src/driver.rs tui-driver/src/error.rs
git commit -m "feat(tui-driver): add mouse click methods"
```

---

### Task 3: Add MCP Mouse Tools

**Files:**
- Modify: `mcp-server/src/tools.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Add tool parameter types**

```rust
#[derive(Debug, Deserialize)]
pub struct ClickParams {
    pub session_id: String,
    pub ref_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ClickAtParams {
    pub session_id: String,
    pub x: u16,
    pub y: u16,
}
```

**Step 2: Add tools to tools/list**

```json
{
    "name": "tui_click",
    "description": "Click on an element by reference ID",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "ref_id": { "type": "string", "description": "Element reference from snapshot" }
        },
        "required": ["session_id", "ref_id"]
    }
},
{
    "name": "tui_click_at",
    "description": "Click at specific coordinates (1-based)",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "x": { "type": "integer", "description": "Column (1-based)" },
            "y": { "type": "integer", "description": "Row (1-based)" }
        },
        "required": ["session_id", "x", "y"]
    }
},
{
    "name": "tui_double_click",
    "description": "Double-click on an element by reference ID",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "ref_id": { "type": "string" }
        },
        "required": ["session_id", "ref_id"]
    }
},
{
    "name": "tui_right_click",
    "description": "Right-click on an element by reference ID",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "ref_id": { "type": "string" }
        },
        "required": ["session_id", "ref_id"]
    }
}
```

**Step 3: Add handlers**

Implement tool_click, tool_click_at, tool_double_click, tool_right_click.

**Step 4: Build and test**

Run: `source ~/.cargo/env && cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add mcp-server/src/tools.rs mcp-server/src/main.rs
git commit -m "feat(mcp-server): add mouse click tools"
```

---

### Task 4: Add Mouse Integration Tests

**Files:**
- Modify: `tui-driver/tests/integration_test.rs`

**Step 1: Add test**

```rust
#[tokio::test]
async fn test_click_at() {
    let options = LaunchOptions::new("bash")
        .args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Click should not error (even if bash doesn't respond to mouse)
    let result = driver.click_at(5, 5);
    assert!(result.is_ok(), "click_at should succeed");

    // Invalid coordinates should error
    let result = driver.click_at(0, 0);
    assert!(result.is_err(), "click_at(0,0) should fail");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}
```

**Step 2: Run tests**

Run: `source ~/.cargo/env && cargo test`

**Step 3: Commit**

```bash
git add tui-driver/tests/integration_test.rs
git commit -m "test(tui-driver): add mouse click integration test"
```

---

### Task 5: Final Verification

**Step 1: Run all checks**

```bash
source ~/.cargo/env
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

**Step 2: Verify tools**

Should now have 14 MCP tools total.
