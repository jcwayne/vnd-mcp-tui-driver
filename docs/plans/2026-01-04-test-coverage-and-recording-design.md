# Test Coverage and Recording Feature Design

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Achieve 85% test coverage for all tools including tui_run_code and JS runtime, add asciicast v3 session recording capability to tui_launch.

**Architecture:** Recording handled by new Recorder struct in tui-driver, integrated into TuiDriver. Tests added as separate test files for run_code and recording features.

**Tech Stack:** Rust, serde_json for asciicast v3, tokio for async, rmcp for MCP testing

---

## 1. Recording Feature

### 1.1 Asciicast v3 Format

Based on [asciinema docs](https://docs.asciinema.org/manual/asciicast/v3/):

**Header (first line):**
```json
{"version":3,"term":{"cols":80,"rows":24},"timestamp":1704371234,"command":"vim"}
```

**Event stream (subsequent lines):**
```json
[0.0,"o","$ "]
[0.5,"i","vim\r"]
[0.2,"o","vim: opening...\r\n"]
[0.0,"r","120x40"]
[5.2,"x",0]
```

Event codes:
- `o` - output (terminal display)
- `i` - input (keyboard, only if includeInput=true)
- `r` - resize (terminal dimension change)
- `x` - exit (session end with exit code)

### 1.2 Recording Parameter Structure

Added to `tui_launch`:

```typescript
recording?: {
  enabled: boolean,      // Whether to record
  outputPath: string,    // Path to .cast file
  includeInput?: boolean // Capture input events (default: false)
}
```

### 1.3 Recorder Implementation

```rust
// tui-driver/src/recording.rs

pub struct RecordingOptions {
    pub enabled: bool,
    pub output_path: String,
    pub include_input: bool,
}

pub struct Recorder {
    file: BufWriter<File>,
    last_event_time: Instant,
    include_input: bool,
}

impl Recorder {
    pub fn new(
        path: &str,
        cols: u16,
        rows: u16,
        command: &str,
        include_input: bool,
    ) -> Result<Self>;

    pub fn record_output(&mut self, data: &str);
    pub fn record_input(&mut self, data: &str);
    pub fn record_resize(&mut self, cols: u16, rows: u16);
    pub fn record_exit(&mut self, code: i32);
}
```

### 1.4 Integration Points

| Method | Event | Capture Point |
|--------|-------|---------------|
| PTY read loop | `o` | After reading from PTY |
| `send_text()` | `i` | Before writing to PTY |
| `press_key()` | `i` | Before writing to PTY |
| `press_keys()` | `i` | Before writing to PTY |
| `resize()` | `r` | After resize completes |
| `close()` | `x` | Before closing PTY |

---

## 2. Expose Missing Launch Parameters

### 2.1 Current State

`LaunchOptions` (tui-driver) has `env` and `cwd` but `LaunchParams` (mcp-server) does not expose them.

### 2.2 Updated LaunchParams

```rust
// mcp-server/src/tools.rs

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RecordingParams {
    pub enabled: bool,
    pub output_path: String,
    #[serde(default)]
    pub include_input: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LaunchParams {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_cols")]
    pub cols: u16,
    #[serde(default = "default_rows")]
    pub rows: u16,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub recording: Option<RecordingParams>,
}
```

---

## 3. Test Coverage

### 3.1 New Test File: run_code_test.rs

**Location:** `mcp-server/tests/run_code_test.rs`

**Test Cases:**

1. `test_run_code_basic` - Execute simple JS, verify result
2. `test_run_code_tui_text` - `tui.text()` returns string
3. `test_run_code_tui_snapshot` - `tui.snapshot()` returns object with span_count
4. `test_run_code_tui_screenshot` - `tui.screenshot()` saves file, returns path
5. `test_run_code_input_methods` - sendText, pressKey, pressKeys work
6. `test_run_code_mouse_methods` - click, clickAt, doubleClick, rightClick, hover, drag
7. `test_run_code_wait_methods` - waitForText, waitForIdle return booleans
8. `test_run_code_control_methods` - resize, sendSignal work
9. `test_run_code_debug_methods` - getScrollback, getInput, getOutput
10. `test_run_code_console_capture` - console.log/warn/error/info/debug in logs
11. `test_run_code_context_persistence` - variable set in call 1 accessible in call 2
12. `test_run_code_error_handling` - invalid JS returns error

### 3.2 New Test File: get_code_interface_test.rs

**Location:** `mcp-server/tests/run_code_test.rs` (same file)

**Test Cases:**

1. `test_get_code_interface_returns_typescript` - Returns non-empty string
2. `test_get_code_interface_contains_methods` - Contains all 18 method signatures
3. `test_get_code_interface_contains_types` - Contains Snapshot, Row, Span types

### 3.3 New Test File: recording_test.rs

**Location:** `mcp-server/tests/recording_test.rs`

**Test Cases:**

1. `test_recording_disabled_by_default` - No file created without recording param
2. `test_recording_creates_file` - File created at outputPath when enabled
3. `test_recording_valid_header` - First line is valid v3 JSON header
4. `test_recording_output_events` - `o` events captured
5. `test_recording_input_events_when_enabled` - `i` events with includeInput=true
6. `test_recording_no_input_events_by_default` - No `i` events with includeInput=false
7. `test_recording_resize_events` - `r` events on resize
8. `test_recording_exit_event` - `x` event on close
9. `test_recording_invalid_path` - Error on invalid path
10. `test_recording_intervals` - Event intervals are positive floats

---

## 4. Documentation Updates

### 4.1 README.md

Add new section:

```markdown
## Session Recording

Record terminal sessions in asciicast v3 format for playback with asciinema:

### Enable Recording

```json
{
  "command": "vim",
  "args": ["file.txt"],
  "recording": {
    "enabled": true,
    "outputPath": "/tmp/session.cast",
    "includeInput": true
  }
}
```

### Play Recording

```bash
asciinema play /tmp/session.cast
```

### Recording Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| enabled | boolean | - | Enable recording |
| outputPath | string | - | Path to save .cast file |
| includeInput | boolean | false | Capture keyboard input |
```

Update tui_launch tool documentation to include:
- `cwd` parameter
- `env` parameter
- `recording` parameter

### 4.2 Tool Descriptions

Update `tui_launch` description in server.rs to mention new parameters.

### 4.3 Module Documentation

Add comprehensive docs to `tui-driver/src/recording.rs`.

---

## 5. Implementation Tasks

### Phase 1: Recording Module

1. Create `tui-driver/src/recording.rs` with Recorder struct
2. Add RecordingOptions to LaunchOptions
3. Integrate Recorder into TuiDriver
4. Hook output capture in PTY read loop
5. Hook input capture in send_text, press_key, press_keys
6. Hook resize capture in resize method
7. Hook exit capture in close method

### Phase 2: MCP Tool Updates

8. Add RecordingParams struct to tools.rs
9. Add cwd, env, recording to LaunchParams
10. Update tui_launch in server.rs to pass new params
11. Update tool description for tui_launch

### Phase 3: Tests

12. Create run_code_test.rs with all test cases
13. Create recording_test.rs with all test cases
14. Verify 85% coverage with cargo tarpaulin

### Phase 4: Documentation

15. Update README.md with recording section
16. Update README.md with cwd/env params
17. Add module docs to recording.rs
18. Verify all tool descriptions are accurate

---

## 6. Success Criteria

- [ ] All existing tests pass
- [ ] New tests for tui_run_code pass (12 tests)
- [ ] New tests for recording pass (10 tests)
- [ ] Test coverage >= 85%
- [ ] Recording files playable with `asciinema play`
- [ ] README.md documents all new features
- [ ] Tool descriptions accurate

---

## 7. Sources

- [asciicast v3 specification](https://docs.asciinema.org/manual/asciicast/v3/)
- [asciinema GitHub](https://github.com/asciinema/asciinema)
