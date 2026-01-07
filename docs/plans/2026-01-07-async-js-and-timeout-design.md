# Async JavaScript and Timeout Design

## Problem

The `tui_run_code` tool hangs when JavaScript code calls `tui.waitForText()` or `tui.waitForIdle()`. This is caused by `futures::executor::block_on()` being called from within a tokio async runtime, causing a deadlock.

## Solution

1. Upgrade Boa from 0.20 to 0.21
2. Use `tokio::task::block_in_place` combined with `Handle::current().block_on()` for async driver calls
3. Add a `timeout` parameter to `tui_run_code` with 60 second default

### Why Not Promises?

Initially, we planned to use `JsPromise::from_async_fn()` to make wait methods return JavaScript Promises. However, this approach had issues:

1. Boa's `Context` is not `Send` (uses `Rc` internally)
2. Async Promises need to be polled by Boa's job executor
3. The default job executor uses `block_on` internally, causing the same deadlock

The solution is to keep wait methods synchronous but use proper tokio integration:
- `block_in_place` tells tokio we're doing blocking work
- `Handle::current().block_on()` properly awaits async driver methods

## Implementation

### 1. mcp-server/Cargo.toml

```toml
boa_engine = "0.21"
```

### 2. mcp-server/src/tools.rs

Added timeout parameter to RunCodeParams:

```rust
fn default_script_timeout() -> u64 {
    60000  // 60 seconds
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunCodeParams {
    pub session_id: String,
    pub code: String,
    #[serde(default = "default_script_timeout")]
    pub timeout: u64,
}
```

### 3. mcp-server/src/boa.rs

The `execute_script` function runs in `block_in_place`:

```rust
pub async fn execute_script(
    driver: &TuiDriver,
    code: &str,
) -> Result<(String, Vec<ConsoleEntry>), String> {
    let driver_ptr = driver as *const TuiDriver;
    let code = code.to_string();

    tokio::task::block_in_place(move || {
        execute_script_blocking(driver_ptr, &code)
    })
}
```

Wait methods use `Handle::current().block_on()`:

```rust
fn create_wait_for_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // ... parse args ...

            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let result = tokio::runtime::Handle::current()
                .block_on(driver.wait_for_text(&text, timeout_ms));

            match result {
                Ok(found) => Ok(JsValue::from(found)),
                Err(e) => Err(JsError::from_opaque(...)),
            }
        }),
    )
    .build()
    .into()
}
```

### 4. mcp-server/src/server.rs

Wrap execution with timeout:

```rust
async fn tui_run_code(&self, params: RunCodeParams) -> Result<CallToolResult, McpError> {
    let timeout_duration = Duration::from_millis(params.timeout);

    match tokio::time::timeout(
        timeout_duration,
        crate::boa::execute_script(driver, &params.code),
    ).await {
        Ok(Ok((result_str, logs))) => { /* success */ },
        Ok(Err(e)) => { /* JS error */ },
        Err(_) => Ok(CallToolResult::error(vec![Content::text(
            format!("Script execution timed out after {}ms", params.timeout)
        )])),
    }
}
```

## TypeScript Interface

All methods are synchronous (blocking):

```typescript
interface Tui {
  // Text/Input
  text(): string;
  sendText(text: string): void;
  pressKey(key: string): void;
  pressKeys(keys: string[]): void;

  // Mouse
  click(ref: string): void;
  clickAt(x: number, y: number): void;
  doubleClick(ref: string): void;
  rightClick(ref: string): void;
  hover(ref: string): void;
  drag(startRef: string, endRef: string): void;

  // Wait (blocking, returns when done or timeout)
  waitForText(text: string, timeoutMs?: number): boolean;
  waitForIdle(timeoutMs?: number, idleMs?: number): boolean;

  // Snapshot
  snapshot(): Snapshot;
  screenshot(filename?: string): string;

  // Control
  resize(cols: number, rows: number): void;
  sendSignal(signal: string): void;

  // Debug
  getScrollback(): number;
  getInput(chars?: number): string;
  getOutput(chars?: number): string;
}
```

## Usage

```javascript
// Wait methods block until condition or timeout
const found = tui.waitForText("hello", 5000);  // blocks up to 5s
tui.waitForIdle(3000);  // blocks up to 3s

// Script-level timeout (default 60s)
// If script takes too long, MCP returns timeout error
```

## Testing

Tests must use multi-threaded tokio runtime because `block_in_place` requires it:

```rust
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_basic() -> anyhow::Result<()> {
    // ...
}
```
