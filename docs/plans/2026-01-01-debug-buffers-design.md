# Debug Buffers Design

## Overview

Add debug buffers to capture I/O history for each TUI session, enabling debugging of terminal interactions.

## Constraints

With PTY (pseudo-terminal), stdout and stderr are merged into a single stream - that's how real terminals work. We cannot separate them at the PTY level.

## Buffers

| Buffer | Storage | Content | Access |
|--------|---------|---------|--------|
| input_buffer | 10000 chars (RingBuffer) | Raw escape sequences sent to process | `get_input_buffer(n)`, `tui_get_input` |
| output_buffer | 10000 chars (RingBuffer) | Raw PTY output with escape codes | `get_output_buffer(n)`, `tui_get_output` |
| scrollback | 500 lines (vt100 built-in) | Parsed text that scrolled off | `get_scrollback()`, `tui_get_scrollback` |

## Data Structures

### RingBuffer

```rust
const BUFFER_CAPACITY: usize = 10000;

struct RingBuffer {
    data: Mutex<VecDeque<char>>,
    capacity: usize,
}

impl RingBuffer {
    fn new(capacity: usize) -> Self;
    fn push_str(&self, s: &str);            // Add chars, evict old if needed
    fn get_last(&self, n: usize) -> String; // Get last N chars
    fn len(&self) -> usize;
    fn clear(&self);
}
```

### TuiDriver Fields

```rust
input_buffer: Arc<RingBuffer>,   // What we sent (raw escape sequences)
output_buffer: Arc<RingBuffer>,  // Raw PTY output
```

### Scrollback

Change vt100 parser initialization from 0 to 500 lines:
```rust
vt100::Parser::new(rows, cols, 500)
```

## Data Flow

### Input Buffer Population

Every method that sends data stores raw bytes:
- `send_text(&str)` - pushes the text as-is
- `press_key(&Key)` - pushes the escape sequence bytes (e.g., `\x1b[A` for arrow up)
- `press_keys(&[Key])` - pushes each key's escape sequence

### Output Buffer Population

The background reader thread stores raw PTY output before parsing:
```rust
// In the reader thread:
let text = String::from_utf8_lossy(&buf[..n]);
output_buffer.push_str(&text);  // Store raw output
parser.process(&buf[..n]);       // Feed to terminal parser
```

### Scrollback

Handled automatically by vt100 once scrollback > 0. Access via `parser.screen().scrollback()`.

## TuiDriver API

```rust
/// Get last N characters from input buffer (raw escape sequences)
pub fn get_input_buffer(&self, n: usize) -> String;

/// Get last N characters from output buffer (raw PTY output)
pub fn get_output_buffer(&self, n: usize) -> String;

/// Get scrollback content (parsed terminal lines)
pub fn get_scrollback(&self) -> String;

/// Clear all debug buffers
pub fn clear_buffers(&self);
```

## MCP Tools

### tui_get_input

Get raw input sent to the process (escape sequences included).

Parameters:
- `session_id` (required): Session identifier
- `chars` (optional, default 10000): Max characters to return

### tui_get_output

Get raw PTY output (escape sequences included).

Parameters:
- `session_id` (required): Session identifier
- `chars` (optional, default 10000): Max characters to return

### tui_get_scrollback

Get parsed terminal lines that scrolled off screen.

Parameters:
- `session_id` (required): Session identifier

## Memory Usage

Per session: ~20KB for char buffers + ~40KB for scrollback = ~60KB max

## Files to Modify

- `tui-driver/src/driver.rs` - Add RingBuffer, new fields, methods
- `tui-driver/src/lib.rs` - Export if needed
- `mcp-server/src/tools.rs` - Add parameter types
- `mcp-server/src/main.rs` - Add 3 MCP tools
