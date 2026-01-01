# Milestone 5: The Scripter - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add JavaScript scripting support using Boa engine for complex automation.

**Architecture:** Boa JS runtime with injected `tui` object that wraps TuiDriver methods.

**Tech Stack:** Rust, boa_engine crate.

---

## Context

The `tui_run_code` tool allows LLMs to execute JavaScript code for complex automation sequences. A `tui` object is injected into the JS runtime with methods matching TuiDriver's API.

Example usage:
```javascript
// Wait for prompt, type command, wait for output
await tui.waitForText("$", 5000);
await tui.sendText("ls -la\n");
await tui.waitForIdle(100, 5000);
return tui.text();
```

---

### Task 1: Add Boa Engine Dependency

**Files:**
- Modify: `mcp-server/Cargo.toml`

**Step 1: Add dependency**

```toml
boa_engine = "0.20"
boa_gc = "0.20"
```

**Step 2: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 3: Commit**

```bash
git add mcp-server/Cargo.toml
git commit -m "chore(mcp-server): add boa_engine dependency for JS scripting"
```

---

### Task 2: Create Boa Module with TUI Object

**Files:**
- Create: `mcp-server/src/boa.rs`

**Step 1: Create boa.rs**

This module will:
1. Create a Boa runtime context
2. Register a `tui` global object with methods
3. Execute JavaScript code and return results

Since Boa doesn't support async/await natively in a simple way, we'll use synchronous wrappers that use tokio's block_on internally, or provide simpler synchronous versions of the methods.

```rust
//! Boa JavaScript runtime integration

use boa_engine::{
    Context, JsError, JsNativeError, JsObject, JsResult, JsValue, NativeFunction, Source,
};
use std::sync::Arc;
use tui_driver::TuiDriver;

/// Execute JavaScript code with TUI context
pub fn execute_script(
    driver: Arc<TuiDriver>,
    code: &str,
    timeout_ms: u64,
) -> Result<String, String> {
    // Create Boa context
    let mut context = Context::default();

    // Create tui object
    let tui = create_tui_object(&mut context, driver)?;

    // Register as global
    context
        .register_global_property(boa_engine::JsString::from("tui"), tui, boa_engine::property::Attribute::all())
        .map_err(|e| e.to_string())?;

    // Execute with timeout (simplified - real timeout would need threads)
    let result = context.eval(Source::from_bytes(code));

    match result {
        Ok(value) => {
            let display = value.to_string(&mut context).map_err(|e| e.to_string())?;
            Ok(display.to_std_string_escaped())
        }
        Err(e) => Err(format!("JavaScript error: {}", e)),
    }
}

fn create_tui_object(context: &mut Context, driver: Arc<TuiDriver>) -> Result<JsValue, String> {
    let tui = JsObject::with_null_proto();

    // Add text() method
    {
        let driver = Arc::clone(&driver);
        let text_fn = NativeFunction::from_copy_closure(move |_this, _args, _ctx| {
            let text = driver.text();
            Ok(JsValue::from(boa_engine::JsString::from(text.as_str())))
        });
        tui.set(
            boa_engine::JsString::from("text"),
            text_fn.to_js_function(context.realm()),
            false,
            context,
        ).map_err(|e| e.to_string())?;
    }

    // Add sendText(text) method
    {
        let driver = Arc::clone(&driver);
        let send_fn = NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let text = args.get_or_undefined(0).to_string(ctx)?;
            driver
                .send_text(&text.to_std_string_escaped())
                .map_err(|e| JsNativeError::error().with_message(e.to_string()))?;
            Ok(JsValue::undefined())
        });
        tui.set(
            boa_engine::JsString::from("sendText"),
            send_fn.to_js_function(context.realm()),
            false,
            context,
        ).map_err(|e| e.to_string())?;
    }

    // Add pressKey(key) method
    {
        let driver = Arc::clone(&driver);
        let fn_impl = NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let key_str = args.get_or_undefined(0).to_string(ctx)?;
            let key = tui_driver::Key::parse(&key_str.to_std_string_escaped())
                .map_err(|e| JsNativeError::error().with_message(e.to_string()))?;
            driver
                .press_key(&key)
                .map_err(|e| JsNativeError::error().with_message(e.to_string()))?;
            Ok(JsValue::undefined())
        });
        tui.set(
            boa_engine::JsString::from("pressKey"),
            fn_impl.to_js_function(context.realm()),
            false,
            context,
        ).map_err(|e| e.to_string())?;
    }

    // Add clickAt(x, y) method
    {
        let driver = Arc::clone(&driver);
        let fn_impl = NativeFunction::from_copy_closure(move |_this, args, ctx| {
            let x = args.get_or_undefined(0).to_u32(ctx)? as u16;
            let y = args.get_or_undefined(1).to_u32(ctx)? as u16;
            driver
                .click_at(x, y)
                .map_err(|e| JsNativeError::error().with_message(e.to_string()))?;
            Ok(JsValue::undefined())
        });
        tui.set(
            boa_engine::JsString::from("clickAt"),
            fn_impl.to_js_function(context.realm()),
            false,
            context,
        ).map_err(|e| e.to_string())?;
    }

    // Add more methods as needed...

    Ok(JsValue::from(tui))
}
```

**Step 2: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 3: Commit**

```bash
git add mcp-server/src/boa.rs
git commit -m "feat(mcp-server): add Boa JS runtime with tui object"
```

---

### Task 3: Add run_code Method to TuiDriver

**Files:**
- Modify: `mcp-server/src/main.rs`

**Step 1: Add module**

```rust
mod boa;
```

**Step 2: Build and verify**

Run: `source ~/.cargo/env && cargo build`

**Step 3: Commit**

```bash
git add mcp-server/src/main.rs
git commit -m "feat(mcp-server): register boa module"
```

---

### Task 4: Add MCP tui_run_code Tool

**Files:**
- Modify: `mcp-server/src/tools.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Add parameter type**

```rust
#[derive(Debug, Deserialize)]
pub struct RunCodeParams {
    pub session_id: String,
    pub code: String,
    #[serde(default = "default_script_timeout")]
    pub timeout_ms: u64,
}

fn default_script_timeout() -> u64 {
    30000
}

#[derive(Debug, Serialize)]
pub struct RunCodeResult {
    pub result: String,
}
```

**Step 2: Add tool to tools/list**

```json
{
    "name": "tui_run_code",
    "description": "Execute JavaScript code with tui object for complex automation",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": { "type": "string" },
            "code": {
                "type": "string",
                "description": "JavaScript code to execute. Use tui.text(), tui.sendText(), tui.pressKey(), etc."
            },
            "timeout_ms": {
                "type": "integer",
                "default": 30000
            }
        },
        "required": ["session_id", "code"]
    }
}
```

**Step 3: Implement handler**

```rust
async fn tool_run_code(&self, id: Value, arguments: Value) -> JsonRpcResponse {
    let params: RunCodeParams = match serde_json::from_value(arguments) {
        Ok(p) => p,
        Err(e) => return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e)),
    };

    let sessions = self.sessions.lock().await;
    match sessions.get(&params.session_id) {
        Some(driver) => {
            // Need to get Arc<TuiDriver> - this requires restructuring session storage
            // For now, we'll return a simplified implementation note
            match crate::boa::execute_script(Arc::new(driver.clone()), &params.code, params.timeout_ms) {
                Ok(result) => {
                    let content = json!({
                        "content": [{"type": "text", "text": result}]
                    });
                    JsonRpcResponse::success(id, content)
                }
                Err(e) => {
                    let content = json!({
                        "content": [{"type": "text", "text": format!("Script error: {}", e)}],
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

Note: This requires making TuiDriver Clone or storing Arc<TuiDriver> in session manager.

**Step 4: Build and test**

Run: `source ~/.cargo/env && cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add mcp-server/src/tools.rs mcp-server/src/main.rs
git commit -m "feat(mcp-server): add tui_run_code tool for JS scripting"
```

---

### Task 5: Update Session Manager for Arc<TuiDriver>

**Files:**
- Modify: `mcp-server/src/main.rs`

**Step 1: Change session storage**

```rust
struct SessionManager {
    sessions: HashMap<String, Arc<TuiDriver>>,
}
```

Update all methods to use Arc<TuiDriver>.

**Step 2: Build and test**

Run: `source ~/.cargo/env && cargo test`

**Step 3: Commit**

```bash
git add mcp-server/src/main.rs
git commit -m "refactor(mcp-server): use Arc<TuiDriver> in session manager"
```

---

### Task 6: Final Verification

**Step 1: Run all checks**

```bash
source ~/.cargo/env
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

**Step 2: Verify tools**

Should now have 15 MCP tools including tui_run_code.
