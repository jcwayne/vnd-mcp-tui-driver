# Milestone 2: The Keyboard - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add keyboard input support with special key mapping and wait utilities.

**Architecture:** Key-to-ANSI mapping module + TuiDriver methods + MCP tools exposure.

**Tech Stack:** Rust, vt100 escape sequences, tokio for async waits.

---

## Context

Milestone 1 already provides:
- `send_text()` - raw text to PTY
- `wait_for_idle()` - wait for screen stability
- `wait_for_text()` - wait for text appearance

Milestone 2 adds:
- `press_key()` - send special key (Enter, Escape, Arrow keys, etc.)
- `press_keys()` - send sequence of keys
- Key-to-ANSI escape sequence mapping
- MCP tools for all keyboard operations

---

### Task 1: Key Mapping Module

**Files:**
- Create: `tui-driver/src/keys.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Create keys.rs with Key enum and ANSI mapping**

```rust
//! Key to ANSI escape sequence mapping

use crate::error::{Result, TuiError};

/// Special keys that can be sent to the terminal
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    // Navigation
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Space,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Page navigation
    Home,
    End,
    PageUp,
    PageDown,

    // Function keys
    F1, F2, F3, F4, F5, F6,
    F7, F8, F9, F10, F11, F12,

    // Modifiers
    Ctrl(char),
    Alt(char),

    // Regular character
    Char(char),
}

impl Key {
    /// Convert key to ANSI escape sequence bytes
    pub fn to_escape_sequence(&self) -> Vec<u8> {
        match self {
            // Simple keys
            Key::Enter => vec![b'\r'],
            Key::Tab => vec![b'\t'],
            Key::Escape => vec![0x1b],
            Key::Backspace => vec![0x7f],
            Key::Delete => vec![0x1b, b'[', b'3', b'~'],
            Key::Insert => vec![0x1b, b'[', b'2', b'~'],
            Key::Space => vec![b' '],

            // Arrow keys (CSI sequences)
            Key::Up => vec![0x1b, b'[', b'A'],
            Key::Down => vec![0x1b, b'[', b'B'],
            Key::Right => vec![0x1b, b'[', b'C'],
            Key::Left => vec![0x1b, b'[', b'D'],

            // Page navigation
            Key::Home => vec![0x1b, b'[', b'H'],
            Key::End => vec![0x1b, b'[', b'F'],
            Key::PageUp => vec![0x1b, b'[', b'5', b'~'],
            Key::PageDown => vec![0x1b, b'[', b'6', b'~'],

            // Function keys
            Key::F1 => vec![0x1b, b'O', b'P'],
            Key::F2 => vec![0x1b, b'O', b'Q'],
            Key::F3 => vec![0x1b, b'O', b'R'],
            Key::F4 => vec![0x1b, b'O', b'S'],
            Key::F5 => vec![0x1b, b'[', b'1', b'5', b'~'],
            Key::F6 => vec![0x1b, b'[', b'1', b'7', b'~'],
            Key::F7 => vec![0x1b, b'[', b'1', b'8', b'~'],
            Key::F8 => vec![0x1b, b'[', b'1', b'9', b'~'],
            Key::F9 => vec![0x1b, b'[', b'2', b'0', b'~'],
            Key::F10 => vec![0x1b, b'[', b'2', b'1', b'~'],
            Key::F11 => vec![0x1b, b'[', b'2', b'3', b'~'],
            Key::F12 => vec![0x1b, b'[', b'2', b'4', b'~'],

            // Ctrl+key (convert to control code)
            Key::Ctrl(c) => {
                let code = (*c as u8).to_ascii_lowercase();
                if code >= b'a' && code <= b'z' {
                    vec![code - b'a' + 1]
                } else {
                    vec![code]
                }
            }

            // Alt+key (ESC prefix)
            Key::Alt(c) => vec![0x1b, *c as u8],

            // Regular character
            Key::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
    }

    /// Parse key from string representation
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "enter" | "return" => Ok(Key::Enter),
            "tab" => Ok(Key::Tab),
            "escape" | "esc" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            "delete" | "del" => Ok(Key::Delete),
            "insert" | "ins" => Ok(Key::Insert),
            "space" => Ok(Key::Space),

            "up" | "arrowup" => Ok(Key::Up),
            "down" | "arrowdown" => Ok(Key::Down),
            "left" | "arrowleft" => Ok(Key::Left),
            "right" | "arrowright" => Ok(Key::Right),

            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" => Ok(Key::PageUp),
            "pagedown" => Ok(Key::PageDown),

            "f1" => Ok(Key::F1),
            "f2" => Ok(Key::F2),
            "f3" => Ok(Key::F3),
            "f4" => Ok(Key::F4),
            "f5" => Ok(Key::F5),
            "f6" => Ok(Key::F6),
            "f7" => Ok(Key::F7),
            "f8" => Ok(Key::F8),
            "f9" => Ok(Key::F9),
            "f10" => Ok(Key::F10),
            "f11" => Ok(Key::F11),
            "f12" => Ok(Key::F12),

            _ => {
                // Check for Ctrl+x or Alt+x
                if let Some(rest) = s.strip_prefix("ctrl+") {
                    if rest.len() == 1 {
                        return Ok(Key::Ctrl(rest.chars().next().unwrap()));
                    }
                }
                if let Some(rest) = s.strip_prefix("alt+") {
                    if rest.len() == 1 {
                        return Ok(Key::Alt(rest.chars().next().unwrap()));
                    }
                }

                // Single character
                if s.len() == 1 {
                    return Ok(Key::Char(s.chars().next().unwrap()));
                }

                Err(TuiError::InvalidKey(s.to_string()))
            }
        }
    }
}
```

**Step 2: Export keys module from lib.rs**

Add to `tui-driver/src/lib.rs`:
```rust
pub mod keys;
pub use keys::Key;
```

**Step 3: Run tests**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add tui-driver/src/keys.rs tui-driver/src/lib.rs
git commit -m "feat(tui-driver): add key to ANSI escape sequence mapping"
```

---

### Task 2: Add press_key and press_keys to TuiDriver

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Add press_key method**

Add to TuiDriver impl:
```rust
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
```

**Step 2: Add import at top of driver.rs**

```rust
use crate::keys::Key;
```

**Step 3: Run build**

Run: `cargo build`
Expected: Compiles successfully

**Step 4: Commit**

```bash
git add tui-driver/src/driver.rs
git commit -m "feat(tui-driver): add press_key and press_keys methods"
```

---

### Task 3: Add Keyboard Integration Tests

**Files:**
- Modify: `tui-driver/tests/integration_test.rs`

**Step 1: Add key press test**

```rust
use tui_driver::Key;

#[tokio::test]
async fn test_press_key() {
    let options =
        LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Type some text using press_keys
    let keys: Vec<Key> = "echo KEYTEST".chars().map(Key::Char).collect();
    driver.press_keys(&keys).expect("Failed to press keys");

    // Press Enter
    driver.press_key(&Key::Enter).expect("Failed to press Enter");

    // Wait for output
    let found = driver
        .wait_for_text("KEYTEST", 2000)
        .await
        .expect("Wait failed");

    assert!(found, "Expected to find KEYTEST in screen");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}

#[tokio::test]
async fn test_key_parse() {
    // Test key parsing
    assert_eq!(Key::parse("Enter").unwrap(), Key::Enter);
    assert_eq!(Key::parse("escape").unwrap(), Key::Escape);
    assert_eq!(Key::parse("ArrowUp").unwrap(), Key::Up);
    assert_eq!(Key::parse("Ctrl+c").unwrap(), Key::Ctrl('c'));
    assert_eq!(Key::parse("Alt+x").unwrap(), Key::Alt('x'));
    assert_eq!(Key::parse("F1").unwrap(), Key::F1);
    assert_eq!(Key::parse("a").unwrap(), Key::Char('a'));

    // Invalid key
    assert!(Key::parse("invalid_key_name").is_err());
}
```

**Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add tui-driver/tests/integration_test.rs
git commit -m "test(tui-driver): add keyboard integration tests"
```

---

### Task 4: Add MCP Keyboard Tools

**Files:**
- Modify: `mcp-server/src/tools.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Add tool parameter types in tools.rs**

```rust
#[derive(Debug, Deserialize)]
pub struct PressKeyParams {
    pub session_id: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct PressKeysParams {
    pub session_id: String,
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SendTextParams {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct WaitForTextParams {
    pub session_id: String,
    pub text: String,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct WaitForIdleParams {
    pub session_id: String,
    #[serde(default = "default_idle_ms")]
    pub idle_ms: u64,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

fn default_idle_ms() -> u64 {
    100
}

#[derive(Debug, Serialize)]
pub struct WaitResult {
    pub found: bool,
}
```

**Step 2: Add tools to tools/list in main.rs**

Add to the tools array in `handle_tools_list`:
```rust
{
    "name": "tui_press_key",
    "description": "Press a single key in a TUI session",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            },
            "key": {
                "type": "string",
                "description": "Key to press (e.g., 'Enter', 'Escape', 'ArrowUp', 'Ctrl+c', 'F1')"
            }
        },
        "required": ["session_id", "key"]
    }
},
{
    "name": "tui_press_keys",
    "description": "Press multiple keys in sequence in a TUI session",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            },
            "keys": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Array of keys to press"
            }
        },
        "required": ["session_id", "keys"]
    }
},
{
    "name": "tui_send_text",
    "description": "Send raw text to a TUI session",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            },
            "text": {
                "type": "string",
                "description": "Text to send"
            }
        },
        "required": ["session_id", "text"]
    }
},
{
    "name": "tui_wait_for_text",
    "description": "Wait for specific text to appear on screen",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            },
            "text": {
                "type": "string",
                "description": "Text to wait for"
            },
            "timeout_ms": {
                "type": "integer",
                "description": "Timeout in milliseconds",
                "default": 5000
            }
        },
        "required": ["session_id", "text"]
    }
},
{
    "name": "tui_wait_for_idle",
    "description": "Wait for the screen to stop updating",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            },
            "idle_ms": {
                "type": "integer",
                "description": "How long screen must be idle",
                "default": 100
            },
            "timeout_ms": {
                "type": "integer",
                "description": "Timeout in milliseconds",
                "default": 5000
            }
        },
        "required": ["session_id"]
    }
}
```

**Step 3: Add tool handlers in main.rs**

Add to `handle_tools_call` match:
```rust
"tui_press_key" => self.tool_press_key(id, arguments).await,
"tui_press_keys" => self.tool_press_keys(id, arguments).await,
"tui_send_text" => self.tool_send_text(id, arguments).await,
"tui_wait_for_text" => self.tool_wait_for_text(id, arguments).await,
"tui_wait_for_idle" => self.tool_wait_for_idle(id, arguments).await,
```

Add handler methods:
```rust
async fn tool_press_key(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: PressKeyParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
        }
    };

    let key = match Key::parse(&params.key) {
        Ok(k) => k,
        Err(e) => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Invalid key: {}", e)}],
                "isError": true
            });
            return JsonRpcResponse::success(id, content);
        }
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            if let Err(e) = driver.press_key(&key) {
                let content = json!({
                    "content": [{"type": "text", "text": format!("Error: {}", e)}],
                    "isError": true
                });
                return JsonRpcResponse::success(id, content);
            }
            let content = json!({
                "content": [{"type": "text", "text": "{\"success\": true}"}]
            });
            JsonRpcResponse::success(id, content)
        }
        None => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Session not found: {}", params.session_id)}],
                "isError": true
            });
            JsonRpcResponse::success(id, content)
        }
    }
}

async fn tool_press_keys(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: PressKeysParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
        }
    };

    let keys: Vec<Key> = match params.keys.iter().map(|s| Key::parse(s)).collect() {
        Ok(k) => k,
        Err(e) => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Invalid key: {}", e)}],
                "isError": true
            });
            return JsonRpcResponse::success(id, content);
        }
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            if let Err(e) = driver.press_keys(&keys) {
                let content = json!({
                    "content": [{"type": "text", "text": format!("Error: {}", e)}],
                    "isError": true
                });
                return JsonRpcResponse::success(id, content);
            }
            let content = json!({
                "content": [{"type": "text", "text": "{\"success\": true}"}]
            });
            JsonRpcResponse::success(id, content)
        }
        None => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Session not found: {}", params.session_id)}],
                "isError": true
            });
            JsonRpcResponse::success(id, content)
        }
    }
}

async fn tool_send_text(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: SendTextParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
        }
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            if let Err(e) = driver.send_text(&params.text) {
                let content = json!({
                    "content": [{"type": "text", "text": format!("Error: {}", e)}],
                    "isError": true
                });
                return JsonRpcResponse::success(id, content);
            }
            let content = json!({
                "content": [{"type": "text", "text": "{\"success\": true}"}]
            });
            JsonRpcResponse::success(id, content)
        }
        None => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Session not found: {}", params.session_id)}],
                "isError": true
            });
            JsonRpcResponse::success(id, content)
        }
    }
}

async fn tool_wait_for_text(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: WaitForTextParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
        }
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            match driver.wait_for_text(&params.text, params.timeout_ms).await {
                Ok(found) => {
                    let result = WaitResult { found };
                    let content = json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&result).unwrap()}]
                    });
                    JsonRpcResponse::success(id, content)
                }
                Err(e) => {
                    let content = json!({
                        "content": [{"type": "text", "text": format!("Error: {}", e)}],
                        "isError": true
                    });
                    JsonRpcResponse::success(id, content)
                }
            }
        }
        None => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Session not found: {}", params.session_id)}],
                "isError": true
            });
            JsonRpcResponse::success(id, content)
        }
    }
}

async fn tool_wait_for_idle(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: WaitForIdleParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
        }
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            match driver.wait_for_idle(params.idle_ms, params.timeout_ms).await {
                Ok(()) => {
                    let content = json!({
                        "content": [{"type": "text", "text": "{\"success\": true}"}]
                    });
                    JsonRpcResponse::success(id, content)
                }
                Err(e) => {
                    let content = json!({
                        "content": [{"type": "text", "text": format!("Error: {}", e)}],
                        "isError": true
                    });
                    JsonRpcResponse::success(id, content)
                }
            }
        }
        None => {
            let content = json!({
                "content": [{"type": "text", "text": format!("Session not found: {}", params.session_id)}],
                "isError": true
            });
            JsonRpcResponse::success(id, content)
        }
    }
}
```

**Step 4: Add import for Key**

Add to imports in main.rs:
```rust
use tui_driver::Key;
```

**Step 5: Run tests**

Run: `cargo test && cargo clippy -- -D warnings`
Expected: All pass

**Step 6: Commit**

```bash
git add mcp-server/src/tools.rs mcp-server/src/main.rs
git commit -m "feat(mcp-server): add keyboard tools (press_key, press_keys, send_text, wait_for_text, wait_for_idle)"
```

---

### Task 5: Final Verification

**Step 1: Run all checks**

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

**Step 2: Test MCP tools list**

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | cargo run --bin mcp-tui-driver 2>/dev/null | jq '.result.tools[].name'
```

Expected output should include all 8 tools:
- tui_launch
- tui_text
- tui_close
- tui_press_key
- tui_press_keys
- tui_send_text
- tui_wait_for_text
- tui_wait_for_idle

**Step 3: Commit any fixes**

If needed, commit fixes.
