# mcp-tui-driver

MCP server for headless TUI automation. Enables LLMs to run, view, and interact with terminal applications.

## Quick Start

```bash
# Build
cargo build --release

# Test MCP handshake
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/mcp-tui-driver
```

## Available Tools

### tui_launch

Launch a new TUI session.

```json
{
  "name": "tui_launch",
  "arguments": {
    "command": "htop",
    "cols": 80,
    "rows": 24
  }
}
```

Returns: `session_id`

### tui_text

Get current text content of a session.

```json
{
  "name": "tui_text",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

Returns: Plain text snapshot of terminal

### tui_close

Close a TUI session.

```json
{
  "name": "tui_close",
  "arguments": {
    "session_id": "<session_id>"
  }
}
```

## Architecture

- `tui-driver/` - Core library for PTY management and terminal emulation
- `mcp-server/` - MCP server binary exposing tools via JSON-RPC over stdio

## Development

```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy
```

## License

MIT
