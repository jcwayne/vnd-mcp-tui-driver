# mcp-tui-driver

MCP server for headless TUI automation. Enables LLMs to run, view, and interact with terminal applications through the Model Context Protocol (MCP).

## Features

- Launch and manage multiple TUI sessions concurrently
- Read terminal content as plain text or accessibility-style snapshots
- Send keyboard input (keys, text, modifier combinations)
- Mouse interaction (click, double-click, right-click)
- Wait for screen content or idle state
- Take PNG screenshots of terminal output
- JavaScript scripting for complex automation workflows
- Session management (resize, signals, info)

## Quick Start

### Build

```bash
cargo build --release
```

### Test MCP Handshake

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/mcp-tui-driver
```

### Basic Usage Example

```bash
# Launch a session
{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"tui_launch","arguments":{"command":"htop"}}}

# Get screen text
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"tui_text","arguments":{"session_id":"<id>"}}}

# Press a key
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"tui_press_key","arguments":{"session_id":"<id>","key":"q"}}}

# Close session
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"tui_close","arguments":{"session_id":"<id>"}}}
```

## Available Tools (19 total)

### Session Management

#### tui_launch

Launch a new TUI application session.

```json
{
  "name": "tui_launch",
  "arguments": {
    "command": "htop",
    "args": [],
    "cols": 80,
    "rows": 24
  }
}
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| command | string | Yes | - | Command to execute |
| args | array | No | [] | Command arguments |
| cols | integer | No | 80 | Terminal width in columns |
| rows | integer | No | 24 | Terminal height in rows |

Returns: `{"session_id": "<uuid>"}`

#### tui_close

Close a TUI session and terminate the process.

```json
{
  "name": "tui_close",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns: `{"success": true}`

#### tui_list_sessions

List all active TUI sessions.

```json
{
  "name": "tui_list_sessions",
  "arguments": {}
}
```

Returns: `{"sessions": ["<id1>", "<id2>", ...]}`

#### tui_get_session

Get information about a TUI session.

```json
{
  "name": "tui_get_session",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns:
```json
{
  "session_id": "<uuid>",
  "command": "htop",
  "cols": 80,
  "rows": 24,
  "running": true
}
```

#### tui_resize

Resize the terminal window dimensions.

```json
{
  "name": "tui_resize",
  "arguments": {
    "session_id": "<session_id>",
    "cols": 120,
    "rows": 40
  }
}
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| session_id | string | Yes | Session identifier |
| cols | integer | Yes | New terminal width in columns |
| rows | integer | Yes | New terminal height in rows |

Returns: `{"success": true}`

#### tui_send_signal

Send a signal to the TUI process.

```json
{
  "name": "tui_send_signal",
  "arguments": {
    "session_id": "<session_id>",
    "signal": "SIGINT"
  }
}
```

Supported signals: `SIGINT`, `SIGTERM`, `SIGKILL`, `SIGHUP`, `SIGQUIT`

Returns: `{"success": true}`

### View

#### tui_text

Get the current plain text content of the terminal.

```json
{
  "name": "tui_text",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns: `{"text": "terminal content here..."}`

#### tui_snapshot

Get an accessibility-style snapshot with element references for clicking.

```json
{
  "name": "tui_snapshot",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns:
```json
{
  "yaml": "- row: 1\n  spans:\n    - ref: span-1\n      text: \"File\"\n      ...",
  "span_count": 42
}
```

The snapshot provides `ref` identifiers (like `span-1`) that can be used with `tui_click`, `tui_double_click`, and `tui_right_click` tools.

#### tui_screenshot

Take a PNG screenshot of the terminal.

```json
{
  "name": "tui_screenshot",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns:
```json
{
  "data": "<base64-encoded-png>",
  "format": "png",
  "width": 640,
  "height": 384
}
```

### Input - Keyboard

#### tui_press_key

Press a single key.

```json
{
  "name": "tui_press_key",
  "arguments": {
    "session_id": "<session_id>",
    "key": "Enter"
  }
}
```

Supported key formats:
- Special keys: `Enter`, `Tab`, `Escape` (or `Esc`), `Backspace`, `Delete`, `Insert`, `Space`
- Arrow keys: `Up`, `Down`, `Left`, `Right` (or `ArrowUp`, `ArrowDown`, etc.)
- Navigation: `Home`, `End`, `PageUp` (or `PgUp`), `PageDown` (or `PgDown`)
- Function keys: `F1` through `F12`
- Ctrl combinations: `Ctrl+c`, `Ctrl+z`, etc.
- Alt combinations: `Alt+x`, `Alt+f`, etc.
- Single characters: `a`, `A`, `1`, `@`, etc.

Returns: `{"success": true}`

#### tui_press_keys

Press multiple keys in sequence.

```json
{
  "name": "tui_press_keys",
  "arguments": {
    "session_id": "<session_id>",
    "keys": ["Down", "Down", "Enter"]
  }
}
```

Returns: `{"success": true}`

#### tui_send_text

Send raw text to the terminal (useful for typing strings).

```json
{
  "name": "tui_send_text",
  "arguments": {
    "session_id": "<session_id>",
    "text": "Hello, World!"
  }
}
```

Returns: `{"success": true}`

### Input - Mouse

#### tui_click

Click on an element by reference ID from the snapshot.

```json
{
  "name": "tui_click",
  "arguments": {
    "session_id": "<session_id>",
    "ref_id": "span-1"
  }
}
```

Returns: `{"success": true}`

#### tui_click_at

Click at specific terminal coordinates.

```json
{
  "name": "tui_click_at",
  "arguments": {
    "session_id": "<session_id>",
    "x": 10,
    "y": 5
  }
}
```

| Parameter | Type | Description |
|-----------|------|-------------|
| x | integer | X coordinate (1-based column) |
| y | integer | Y coordinate (1-based row) |

Returns: `{"success": true}`

#### tui_double_click

Double-click on an element by reference ID.

```json
{
  "name": "tui_double_click",
  "arguments": {
    "session_id": "<session_id>",
    "ref_id": "span-1"
  }
}
```

Returns: `{"success": true}`

#### tui_right_click

Right-click on an element by reference ID.

```json
{
  "name": "tui_right_click",
  "arguments": {
    "session_id": "<session_id>",
    "ref_id": "span-1"
  }
}
```

Returns: `{"success": true}`

### Wait

#### tui_wait_for_text

Wait for specific text to appear on the screen.

```json
{
  "name": "tui_wait_for_text",
  "arguments": {
    "session_id": "<session_id>",
    "text": "Ready",
    "timeout_ms": 5000
  }
}
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| session_id | string | Yes | - | Session identifier |
| text | string | Yes | - | Text to wait for |
| timeout_ms | integer | No | 5000 | Timeout in milliseconds |

Returns: `{"found": true}` or `{"found": false}` if timeout

#### tui_wait_for_idle

Wait for the screen to stop changing (become idle).

```json
{
  "name": "tui_wait_for_idle",
  "arguments": {
    "session_id": "<session_id>",
    "idle_ms": 100,
    "timeout_ms": 5000
  }
}
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| session_id | string | Yes | - | Session identifier |
| idle_ms | integer | No | 100 | How long screen must be stable |
| timeout_ms | integer | No | 5000 | Timeout in milliseconds |

Returns: `{"success": true}`

### Scripting

#### tui_run_code

Execute JavaScript code with access to TUI automation functions.

```json
{
  "name": "tui_run_code",
  "arguments": {
    "session_id": "<session_id>",
    "code": "tui.sendText('hello'); tui.pressKey('Enter'); tui.text()"
  }
}
```

Available `tui` object methods:
- `tui.text()` - Returns the current screen text
- `tui.sendText(text)` - Sends text to the terminal
- `tui.pressKey(key)` - Presses a key (e.g., "Enter", "Ctrl+c")
- `tui.clickAt(x, y)` - Clicks at the specified coordinates
- `tui.snapshot()` - Returns a YAML accessibility snapshot

Returns: `{"result": "<last expression value>"}`

## Architecture

```
mcp-tui-driver/
  tui-driver/       # Core library for PTY management and terminal emulation
    src/
      driver.rs     # TuiDriver - main automation interface
      keys.rs       # Key parsing and ANSI escape sequences
      mouse.rs      # Mouse event handling
      snapshot.rs   # Accessibility snapshot generation and screenshot rendering
      error.rs      # Error types
      lib.rs        # Public API exports
  mcp-server/       # MCP server binary exposing tools via JSON-RPC over stdio
    src/
      main.rs       # MCP protocol handling and tool dispatch
      tools.rs      # Tool parameter and result types
      boa.rs        # JavaScript runtime integration (Boa engine)
```

### Key Dependencies

- **alacritty_terminal** - Terminal emulation
- **rustix** - PTY (pseudo-terminal) management
- **boa_engine** - JavaScript execution for scripting
- **image/ab_glyph** - Screenshot rendering with font support

## Development

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy

# Build release binary
cargo build --release
```

### Environment Variables

- `RUST_LOG` - Set logging level (e.g., `debug`, `info`, `mcp_tui_driver=debug`)

### Running Integration Tests

Integration tests require a functional PTY environment:

```bash
cargo test --package tui-driver -- --test-threads=1
```

## MCP Configuration

Add to your MCP client configuration:

```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "/path/to/mcp-tui-driver",
      "args": []
    }
  }
}
```

## Protocol

The server communicates over stdin/stdout using JSON-RPC 2.0 with the MCP protocol (version 2024-11-05).

### Initialization

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
```

### List Tools

```json
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

### Call Tool

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"<tool_name>","arguments":{...}}}
```

## License

MIT
