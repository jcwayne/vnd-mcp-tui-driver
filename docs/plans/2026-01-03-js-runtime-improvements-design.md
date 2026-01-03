# JavaScript Runtime Improvements Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix `snapshot()` bug, expand JS runtime to support all MCP tools, persist context per-session, add `tui_get_code_interface` tool, and add console logging.

**Architecture:** Per-session JavaScript context stored alongside TuiDriver, lazy-initialized on first `run_code` call. All 18 non-session-management tools exposed on the `tui` object. Console methods capture logs returned with results.

**Tech Stack:** Boa JavaScript engine, rmcp, serde for serialization

---

## 1. Per-Session JavaScript Context

### Storage Structure

```rust
// In mcp-server/src/server.rs
pub struct SessionState {
    driver: TuiDriver,
    js_context: Option<boa_engine::Context>,  // Lazy-initialized
}
```

### Lifecycle

- Context created on first `tui_run_code` call for that session
- All subsequent `tui_run_code` calls reuse the same context (variables persist)
- Context destroyed when `tui_close` is called
- The existing `Mutex<HashMap<...>>` serializes access (Boa Context isn't thread-safe)

### Initialization

On first use, create context and register:
- `tui` object with all 18 methods
- `console` object with `log`, `warn`, `error`, `info`, `debug`

---

## 2. New Tool: tui_get_code_interface

Returns dynamically-generated TypeScript definitions describing the full `tui` API.

### Tool Definition

```rust
#[tool(description = "Get TypeScript interface definitions for tui_run_code. Call this before using tui_run_code to understand the available API.")]
async fn tui_get_code_interface(&self) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(
        generate_typescript_interface()
    )]))
}
```

### Generated Interface

```typescript
interface Tui {
  // Display
  text(): string;
  snapshot(): Snapshot;
  screenshot(filename?: string): string;  // Returns file path

  // Input
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

  // Wait
  waitForText(text: string, timeoutMs?: number): boolean;
  waitForIdle(timeoutMs?: number, idleMs?: number): boolean;

  // Control
  resize(cols: number, rows: number): void;
  sendSignal(signal: string): void;

  // Debug
  getScrollback(): number;
  getInput(chars?: number): string;
  getOutput(chars?: number): string;
}

interface Snapshot {
  rows: Row[];
  spans: Span[];
  span_count: number;
}

interface Row {
  row_number: number;
  spans: Span[];
}

interface Span {
  ref: string;
  text: string;
  x: number;
  y: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  underline_style?: string;
  strikethrough?: boolean;
  blink?: string;
  fg?: string;
  bg?: string;
  link?: string;
}

declare const tui: Tui;
declare const console: Console;
```

---

## 3. Method Return Types

| Method | Return Type | Notes |
|--------|-------------|-------|
| `text()` | `string` | Plain text content |
| `snapshot()` | `Snapshot` | Parsed JS object, not YAML string |
| `screenshot(filename?)` | `string` | File path to PNG |
| `sendText(text)` | `void` | |
| `pressKey(key)` | `void` | |
| `pressKeys(keys)` | `void` | |
| `click(ref)` | `void` | |
| `clickAt(x, y)` | `void` | |
| `doubleClick(ref)` | `void` | |
| `rightClick(ref)` | `void` | |
| `hover(ref)` | `void` | |
| `drag(start, end)` | `void` | |
| `waitForText(text, timeout?)` | `boolean` | true if found |
| `waitForIdle(timeout?, idle?)` | `boolean` | true if idle |
| `resize(cols, rows)` | `void` | |
| `sendSignal(signal)` | `void` | |
| `getScrollback()` | `number` | Lines scrolled off |
| `getInput(chars?)` | `string` | Raw input buffer |
| `getOutput(chars?)` | `string` | Raw output buffer |

---

## 4. Console Capture

### Console Object

```rust
fn create_console_object(context: &mut Context, logs: Arc<Mutex<Vec<ConsoleEntry>>>) -> JsValue {
    // console.log, console.warn, console.error, console.info, console.debug
    // Each captures level and joins args with space
}
```

### Return Structure

```rust
#[derive(Serialize)]
pub struct RunCodeResult {
    result: String,  // JSON-serialized return value
    logs: Vec<ConsoleEntry>,
}

#[derive(Serialize)]
pub struct ConsoleEntry {
    level: String,   // "log" | "warn" | "error" | "info" | "debug"
    message: String,
}
```

### Example Response

```json
{
  "result": "{\"span_count\":42,\"rows\":[...]}",
  "logs": [
    {"level": "log", "message": "Starting automation"},
    {"level": "warn", "message": "Element not found, retrying"}
  ]
}
```

---

## 5. Screenshot File Handling

### Directory Structure

```
/tmp/tui-screenshots/
  <session-id>/
    screenshot-1704271234567.png
    screenshot-1704271235123.png
```

### Behavior

- `tui.screenshot()` - Auto-generates timestamped filename, returns full path
- `tui.screenshot("myshot.png")` - Uses provided name, returns full path
- Directory created on first screenshot call per session
- Files persist after session closes for agent retrieval

---

## 6. Documentation Updates

### Files to Update

1. **README.md** - Update `tui_run_code` description, add `tui_get_code_interface` tool
2. **mcp-server/src/boa.rs** - Update module doc comments with full API
3. **server.rs tool descriptions** - Update `tui_run_code` to mention persistent context

### Tool Description Update

The `tui_run_code` tool description should include:

> "Execute JavaScript code with access to the tui object for complex automation.
> Call `tui_get_code_interface` first to get TypeScript definitions.
> Variables persist across calls within the same session."

---

## 7. Implementation Tasks

### Phase 1: Fix Bug and Restructure

1. Create `SessionState` struct wrapping `TuiDriver` + optional JS context
2. Update `sessions` HashMap to use `SessionState`
3. Fix `snapshot()` to return parsed JS object instead of YAML string

### Phase 2: Expand JS Methods

4. Add all 18 methods to `tui` object in `boa.rs`
5. Implement proper return type conversion for each method
6. Add screenshot file writing logic

### Phase 3: Console and Interface

7. Implement `console` object with log capture
8. Update `RunCodeResult` to include logs
9. Add `tui_get_code_interface` tool with TypeScript generation

### Phase 4: Documentation

10. Update README.md
11. Update tool descriptions in server.rs
12. Update boa.rs module documentation

---

## 8. Testing

### Unit Tests

- TypeScript interface generation produces valid output
- Console log capture works correctly
- Each tui method returns correct type

### Integration Tests

- Variables persist across `run_code` calls
- `snapshot()` returns parseable object
- `screenshot()` creates file and returns valid path
- Console logs appear in response

### Manual Tests

- Test with vim, nvim, lazygit using `run_code` scripts
- Verify `tui_get_code_interface` output is useful for LLMs
