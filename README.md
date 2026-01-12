# mcp-tui-driver

Playwright MCP, but for TUI apps.

MCP server for headless TUI automation. Enables LLMs to run, view, and interact with terminal applications through the Model Context Protocol (MCP).

## Quick installation

```sh
cargo install --git https://github.com/michaellee8/mcp-tui-driver
```

## MCP Configuration

<details>
<summary><b>Claude Code (CLI)</b></summary>

Edit `~/.claude/settings.json`:

```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "mcp-tui-driver",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>Claude Desktop</b></summary>

Edit the config file:
- **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "/path/to/.cargo/bin/mcp-tui-driver",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>Cursor</b></summary>

Edit `~/.cursor/mcp.json` (or `.cursor/mcp.json` in your project root):

```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "mcp-tui-driver",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>VS Code (Continue Extension)</b></summary>

Edit `~/.continue/config.json`:

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "transport": {
          "type": "stdio",
          "command": "mcp-tui-driver",
          "args": []
        }
      }
    ]
  }
}
```

</details>

<details>
<summary><b>Windsurf (Codeium)</b></summary>

Edit `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "mcp-tui-driver",
      "args": []
    }
  }
}
```

</details>

<details>
<summary><b>OpenAI Codex CLI</b></summary>

Edit `~/.codex/config.json`:

```json
{
  "mcp_servers": {
    "tui-driver": {
      "command": "mcp-tui-driver"
    }
  }
}
```

</details>

<details>
<summary><b>Troubleshooting</b></summary>

- **Binary not found**: Use the full path (e.g., `/Users/<username>/.cargo/bin/mcp-tui-driver`)
- **Permission denied**: Run `chmod +x $(which mcp-tui-driver)`
- **Debug logging**: Add `"env": {"RUST_LOG": "debug"}` to the server config

</details>

## Features

- Launch and manage multiple TUI sessions concurrently
- Read terminal content as plain text or accessibility-style snapshots
- Send keyboard input (keys, text, modifier combinations)
- Mouse interaction (click, double-click, right-click)
- Wait for screen content or idle state
- Take PNG screenshots of terminal output
- JavaScript scripting for complex automation workflows
- Session management (resize, signals, info)
- Session recording to asciicast format for playback with asciinema

## Session Recording

TUI sessions can be recorded to asciicast v3 format files (.cast) for later playback with asciinema or other compatible players.

### Enabling Recording

To enable recording, pass a `recording` configuration when launching a session:

```json
{
  "name": "tui_launch",
  "arguments": {
    "command": "bash",
    "recording": {
      "enabled": true,
      "outputPath": "/tmp/session.cast",
      "includeInput": false
    }
  }
}
```

### Recording Options

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| enabled | boolean | Yes | - | Whether recording is enabled |
| outputPath | string | Yes | - | Path to write the recording file (.cast extension recommended) |
| includeInput | boolean | No | false | Whether to include input events in the recording |

### Playing Recordings

Recordings can be played back using the asciinema CLI:

```bash
asciinema play /tmp/session.cast
```

Or uploaded to asciinema.org for web playback:

```bash
asciinema upload /tmp/session.cast
```

### Recording Format

Recordings use the asciicast v3 format, which consists of:
- A JSON header line with version, terminal dimensions, timestamp, and command
- Event lines in the format `[interval, "type", "data"]`

Event types:
- `o` - Output data (terminal output)
- `i` - Input data (user input, if `includeInput` is enabled)
- `r` - Resize event (terminal dimension changes)
- `x` - Exit event (process termination with exit code)

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

## Available Tools (23 total)

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
| cwd | string | No | current dir | Working directory for the command |
| env | object | No | {} | Additional environment variables (merged with existing) |
| recording | object | No | null | Recording configuration (see Session Recording) |

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

#### tui_get_code_interface

Get TypeScript interface definitions for `tui_run_code`. Call this before using `tui_run_code` to understand the available API.

```json
{
  "name": "tui_get_code_interface",
  "arguments": {}
}
```

Returns: TypeScript interface definitions as plain text, including the `Tui` interface with all available methods and the `Console` interface.

#### tui_run_code

Execute JavaScript code with access to TUI automation functions. Call `tui_get_code_interface` first to get TypeScript definitions for the available API.

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

**Display:**
- `tui.text()` - Returns the current screen text
- `tui.snapshot()` - Returns an accessibility snapshot as a JavaScript object
- `tui.screenshot(filename?)` - Takes a screenshot and saves to file, returns the file path

**Input:**
- `tui.sendText(text)` - Sends text to the terminal
- `tui.pressKey(key)` - Presses a key (e.g., "Enter", "Ctrl+c")
- `tui.pressKeys(keys)` - Presses multiple keys in sequence

**Mouse:**
- `tui.click(ref)` - Clicks on element by reference ID
- `tui.clickAt(x, y)` - Clicks at the specified coordinates
- `tui.doubleClick(ref)` - Double-clicks on element by reference ID
- `tui.rightClick(ref)` - Right-clicks on element by reference ID
- `tui.hover(ref)` - Hovers over element by reference ID
- `tui.drag(startRef, endRef)` - Drags from one element to another

**Wait:**
- `tui.waitForText(text, timeoutMs?)` - Waits for text to appear, returns boolean
- `tui.waitForIdle(timeoutMs?, idleMs?)` - Waits for screen to settle, returns boolean

**Control:**
- `tui.resize(cols, rows)` - Resizes the terminal
- `tui.sendSignal(signal)` - Sends a signal (SIGINT, SIGTERM, etc.)

**Debug:**
- `tui.getScrollback()` - Returns number of lines scrolled off screen
- `tui.getInput(chars?)` - Returns raw input buffer
- `tui.getOutput(chars?)` - Returns raw output buffer

**Console output** is captured and returned with results:
- `console.log(...)`, `console.info(...)`, `console.warn(...)`, `console.error(...)`, `console.debug(...)`

Returns:
```json
{
  "result": "<last expression value>",
  "logs": [{"level": "log", "message": "..."}, ...]
}
```

### Debug

#### tui_get_input

Get raw input sent to the process (escape sequences included). Useful for debugging what was sent to the terminal.

```json
{
  "name": "tui_get_input",
  "arguments": {
    "session_id": "<session_id>",
    "chars": 10000
  }
}
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| session_id | string | Yes | - | Session identifier |
| chars | integer | No | 10000 | Maximum characters to return |

Returns:
```json
{
  "length": 1234,
  "content": "<raw escape sequences>"
}
```

#### tui_get_output

Get raw PTY output (escape sequences included). Useful for debugging terminal output.

```json
{
  "name": "tui_get_output",
  "arguments": {
    "session_id": "<session_id>",
    "chars": 10000
  }
}
```

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| session_id | string | Yes | - | Session identifier |
| chars | integer | No | 10000 | Maximum characters to return |

Returns:
```json
{
  "length": 5678,
  "content": "<raw PTY output>"
}
```

#### tui_get_scrollback

Get the number of lines that have scrolled off the visible screen.

```json
{
  "name": "tui_get_scrollback",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns:
```json
{
  "lines": 42
}
```

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
      main.rs       # CLI entrypoint and transport setup
      server.rs     # MCP protocol handling and tool implementations
      tools.rs      # Tool parameter and result types
      boa.rs        # JavaScript runtime integration (Boa engine)
```

### Key Dependencies

- **wezterm-term** - Terminal emulation from WezTerm
- **portable-pty** - Cross-platform PTY (pseudo-terminal) management
- **boa_engine** - JavaScript execution for scripting
- **rmcp** - MCP protocol implementation
- **image** - Screenshot rendering

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
