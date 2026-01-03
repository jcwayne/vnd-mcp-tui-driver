# wezterm-term + rmcp Migration Design

## Overview

Modernize mcp-tui-driver by replacing hand-rolled components with maintained libraries:

- **Phase 1:** Replace `vt100` with `wezterm-term` for terminal emulation
- **Phase 2:** Replace hand-rolled JSON-RPC with `rmcp` SDK for MCP protocol

## Dependency Changes

### Phase 1: tui-driver/Cargo.toml

```toml
# Remove
vt100 = "0.15"

# Add
wezterm-term = { git = "https://github.com/wezterm/wezterm", tag = "20240203-110809-5046fc22" }
```

### Phase 2: mcp-server/Cargo.toml

```toml
# Add
rmcp = { version = "...", features = ["server", "transport-sse"] }
clap = { version = "4", features = ["derive"] }
```

---

## Phase 1: wezterm-term Migration

### Files to Modify

| File | Changes |
|------|---------|
| `Cargo.toml` | Replace vt100 dep with wezterm-term git dep |
| `driver.rs` | Replace `vt100::Parser` with `wezterm_term::Terminal` |
| `snapshot.rs` | Update cell/color APIs, add new feature extraction |

### API Mapping

```rust
// Current (vt100)                    // New (wezterm-term)
vt100::Parser::new(rows, cols, sc)    Terminal::new(TerminalSize { rows, cols, ... })
parser.process(&bytes)                 terminal.advance_bytes(&bytes)
parser.screen()                        terminal.screen()
screen.cell(row, col)                  screen.get_cell(col, row)  // note: x,y order differs
cell.contents()                        cell.str()
cell.fgcolor()                         cell.attrs().foreground()
cell.bold()                            cell.attrs().intensity() == Intensity::Bold
```

### New Features to Extract

- `cell.attrs().hyperlink()` - Optional URL
- `cell.attrs().underline()` - Underline style (None, Single, Double, Curly)
- `cell.attrs().strikethrough()` - bool
- `cell.attrs().italic()` - bool
- `cell.attrs().blink()` - Blink style
- Image detection via cell content inspection

---

## Extended Snapshot Format

### Current Format

```yaml
- row 1:
  - span "Hello"[ref=s1] (1,1)
  - span "World" [bold, fg=color2][ref=s2] (7,1)
```

### Extended Format

```yaml
- row 1:
  - span "Click here"[ref=s1, link=https://example.com] (1,1)
  - span "styled" [bold, italic, strikethrough, underline=curly, fg=color2][ref=s2] (12,1)
  - span "[IMAGE]"[ref=s3, image=img-1, size=40x10, alt="graph"] (20,1)
  - span "blinking" [blink=slow][ref=s4] (30,1)
```

### New Span Attributes

| Attribute | Values | Example |
|-----------|--------|---------|
| `link` | URL string | `link=https://...` |
| `image` | Image ref ID | `image=img-1` |
| `size` | WxH in cells | `size=40x10` |
| `alt` | Alt text | `alt="description"` |
| `italic` | flag | `italic` |
| `strikethrough` | flag | `strikethrough` |
| `underline` | `single\|double\|curly` | `underline=double` |
| `blink` | `slow\|rapid` | `blink=slow` |

### Image Handling

- Detect image cells in terminal screen
- Generate unique `img-N` reference IDs
- Store image bounds (position + size)
- AI agents use `tui_screenshot` with element ref to capture specific images

---

## Phase 2: rmcp Migration

### New Server Structure

```rust
use rmcp::{tool, tool_router, handler::server::tool::ToolRouter};
use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// Run in SSE mode instead of stdio
    #[arg(long)]
    sse: bool,

    /// Port for SSE server
    #[arg(long, default_value = "8080")]
    port: u16,
}

#[derive(Clone)]
pub struct TuiServer {
    sessions: Arc<Mutex<HashMap<String, TuiDriver>>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TuiServer {
    #[tool(description = "Launch a new TUI application session")]
    async fn tui_launch(&self, params: LaunchParams) -> Result<CallToolResult, McpError> { ... }

    #[tool(description = "Get current text content")]
    async fn tui_text(&self, params: SessionParams) -> Result<CallToolResult, McpError> { ... }

    // ... 20 more tools
}
```

### Tool Categories

| Category | Tools |
|----------|-------|
| Session | `tui_launch`, `tui_close`, `tui_list_sessions`, `tui_get_session` |
| Display | `tui_text`, `tui_snapshot`, `tui_screenshot` |
| Input | `tui_press_key`, `tui_press_keys`, `tui_send_text` |
| Mouse | `tui_click`, `tui_click_at`, `tui_double_click`, `tui_right_click` |
| Wait | `tui_wait_for_text`, `tui_wait_for_idle` |
| Control | `tui_resize`, `tui_send_signal` |
| Script | `tui_run_code` |
| Debug | `tui_get_input`, `tui_get_output`, `tui_get_scrollback` |

### Transport & CLI

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let server = TuiServer::new();

    if cli.sse {
        rmcp::serve_sse(server, cli.port).await?;
    } else {
        rmcp::serve_stdio(server).await?;
    }

    Ok(())
}
```

**Usage:**

```bash
# Default (stdio) - for Claude Code
mcp-tui-driver

# SSE mode - for web clients
mcp-tui-driver --sse --port 3000
```

---

## Testing Strategy

### Existing Tests to Keep (57 total)

| Location | Tests | Action |
|----------|-------|--------|
| `tui-driver/src/keys.rs` | 16 key parsing tests | Keep as-is |
| `tui-driver/src/mouse.rs` | 15 mouse tests | Keep as-is |
| `tui-driver/src/snapshot.rs` | 16 snapshot tests | Update for new APIs |
| `tests/integration_test.rs` | 8 integration tests | Update for wezterm-term |
| Doc tests | 2 | Update examples |

### New rmcp Test Suite

```rust
// tests/rmcp_tools_test.rs
use rmcp::testing::TestClient;

#[tokio::test]
async fn test_launch_tool() {
    let server = TuiServer::new();
    let client = TestClient::new(server);

    let result = client.call_tool("tui_launch", json!({
        "command": "echo",
        "args": ["hello"]
    })).await;

    assert!(result.is_ok());
}
```

**Test categories:**
- Tool invocation (each of 22 tools)
- Error handling (invalid params, missing session)
- Transport (stdio and SSE modes)
- New features (hyperlinks, images, extended attributes)

---

## Implementation Order

### Phase 1: wezterm-term

| Step | Task | Files |
|------|------|-------|
| 1.1 | Add wezterm-term git dep | `tui-driver/Cargo.toml` |
| 1.2 | Update `TuiDriver` to use `wezterm_term::Terminal` | `driver.rs` |
| 1.3 | Update snapshot cell/color extraction | `snapshot.rs` |
| 1.4 | Add hyperlink extraction | `snapshot.rs` |
| 1.5 | Add image placeholder detection | `snapshot.rs` |
| 1.6 | Add extended attributes (italic, strikethrough, etc.) | `snapshot.rs` |
| 1.7 | Update existing tests | `snapshot.rs`, `integration_test.rs` |
| 1.8 | Remove vt100 dependency | `Cargo.toml` |

### Phase 2: rmcp

| Step | Task | Files |
|------|------|-------|
| 2.1 | Add rmcp + clap deps | `mcp-server/Cargo.toml` |
| 2.2 | Create `TuiServer` struct with `#[tool_router]` | `main.rs` |
| 2.3 | Migrate all 22 tools to `#[tool]` methods | `main.rs` |
| 2.4 | Add CLI parsing with clap | `main.rs` |
| 2.5 | Implement stdio transport | `main.rs` |
| 2.6 | Implement SSE transport | `main.rs` |
| 2.7 | Remove old JSON-RPC code | `main.rs` |
| 2.8 | Add new rmcp test suite | `tests/rmcp_tools_test.rs` |

### Estimated Line Count Changes

| File | Before | After | Change |
|------|--------|-------|--------|
| `mcp-server/src/main.rs` | 1900 | ~600 | -70% |
| `tui-driver/src/driver.rs` | ~640 | ~700 | +10% |
| `tui-driver/src/snapshot.rs` | ~850 | ~950 | +12% |
