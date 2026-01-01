# mcp-tui-driver Design Document

A Playwright-like MCP server for headless TUI automation, enabling LLMs to run, view, and interact with terminal applications.

## Overview

**Goal:** Create an MCP server that allows LLMs to automate terminal applications (vim, htop, lazygit, k9s, etc.) using accessibility-style snapshots and element references, similar to how Playwright MCP works for web browsers.

**Core Philosophy:**
- Treat the terminal grid as a simplified DOM
- Provide span-based snapshots (not raw text streams)
- Let the LLM decide what's interactive (no over-engineered heuristics)
- Offer both simple MCP tools and JavaScript scripting for complex automation

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  LLM / MCP Client                                           │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ JSON-RPC (stdio)
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  mcp-server (binary)                                        │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  MCP Tools (rmcp)                                   │    │
│  │  - tui_launch, tui_snapshot, tui_click, etc.        │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Session Manager                                    │    │
│  │  HashMap<SessionId, TuiDriver>                      │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  Boa JavaScript Runtime                             │    │
│  │  For tui_run_code complex automation                │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  tui-driver (library)                                       │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │ portable-pty │  │   vt100      │  │  Snapshot    │       │
│  │ PTY spawn/IO │  │   Parser     │  │  Generator   │       │
│  └──────────────┘  └──────────────┘  └──────────────┘       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  Target Application (htop, vim, lazygit, etc.)              │
└─────────────────────────────────────────────────────────────┘
```

## Canonical TypeScript Interface

This interface defines both the MCP tools and the Boa scripting API:

```typescript
/** Session configuration for launching a TUI */
interface LaunchOptions {
  command: string;
  args?: string[];
  cols?: number;       // default: 80
  rows?: number;       // default: 24
  env?: Record<string, string>;
  cwd?: string;
}

/** Element reference from accessibility snapshot */
interface ElementRef {
  ref: string;         // e.g., "s1", "s2"
  text: string;
  x: number;           // 1-based column
  y: number;           // 1-based row
  width: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  inverse?: boolean;
  fg?: string;
  bg?: string;
}

/** Accessibility snapshot result */
interface Snapshot {
  yaml: string;
  spans: ElementRef[];
  getByRef(ref: string): ElementRef | null;
  getByText(text: string): ElementRef | null;
}

/** Screenshot result */
interface Screenshot {
  data: string;        // Base64-encoded
  format: "png" | "jpeg";
  width: number;
  height: number;
}

/** Session info */
interface SessionInfo {
  sessionId: string;
  command: string;
  cols: number;
  rows: number;
  pid: number;
  running: boolean;
  createdAt: string;
}

type Signal = "SIGINT" | "SIGTERM" | "SIGHUP" | "SIGKILL" | "SIGQUIT";

type SpecialKey =
  | "Enter" | "Tab" | "Escape" | "Backspace" | "Delete"
  | "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight"
  | "Home" | "End" | "PageUp" | "PageDown"
  | "F1" | "F2" | "F3" | "F4" | "F5" | "F6"
  | "F7" | "F8" | "F9" | "F10" | "F11" | "F12"
  | "Insert" | "Space";

type Key = SpecialKey | `Ctrl+${string}` | `Alt+${string}` | `Shift+${SpecialKey}`;

interface WaitOptions {
  text?: string;
  ref?: string;
  idle?: number;
  timeout?: number;
  interval?: number;
}

/**
 * TUI Driver API
 *
 * Available as:
 * - MCP tools: tui_launch, tui_snapshot, tui_click, etc.
 * - Boa scripting: `tui.launch()`, `tui.snapshot()`, etc.
 */
interface TuiDriver {
  // Session Management
  launch(options: LaunchOptions): Promise<string>;
  close(sessionId: string): Promise<void>;
  resize(sessionId: string, cols: number, rows: number): Promise<void>;
  listSessions(): Promise<SessionInfo[]>;
  getSession(sessionId: string): Promise<SessionInfo>;
  sendSignal(sessionId: string, signal: Signal): Promise<void>;

  // Snapshots
  snapshot(sessionId: string): Promise<Snapshot>;
  text(sessionId: string): Promise<string>;
  screenshot(sessionId: string, format?: "png" | "jpeg"): Promise<Screenshot>;

  // Element-based Input
  click(sessionId: string, ref: string): Promise<void>;
  doubleClick(sessionId: string, ref: string): Promise<void>;
  rightClick(sessionId: string, ref: string): Promise<void>;
  type(sessionId: string, text: string, options?: {
    ref?: string;
    submit?: boolean;
    delay?: number;
  }): Promise<void>;
  focus(sessionId: string, ref: string): Promise<void>;

  // Raw Input
  pressKey(sessionId: string, key: Key): Promise<void>;
  pressKeys(sessionId: string, keys: Key[]): Promise<void>;
  sendText(sessionId: string, text: string): Promise<void>;
  rawWrite(sessionId: string, data: Uint8Array | string): Promise<void>;
  clickAt(sessionId: string, x: number, y: number): Promise<void>;

  // Waiting
  waitFor(sessionId: string, options: WaitOptions): Promise<boolean>;
  waitForText(sessionId: string, text: string, timeout?: number): Promise<boolean>;
  waitForIdle(sessionId: string, idleTime?: number, timeout?: number): Promise<void>;

  // Scripting
  runCode(sessionId: string, code: string): Promise<unknown>;
}

/**
 * Session-bound API (injected into Boa scripts as `tui`)
 */
interface TuiSession {
  readonly sessionId: string;
  readonly cols: number;
  readonly rows: number;

  snapshot(): Promise<Snapshot>;
  text(): Promise<string>;
  screenshot(format?: "png" | "jpeg"): Promise<Screenshot>;

  click(ref: string): Promise<void>;
  doubleClick(ref: string): Promise<void>;
  rightClick(ref: string): Promise<void>;
  type(text: string, options?: { ref?: string; submit?: boolean; delay?: number }): Promise<void>;
  focus(ref: string): Promise<void>;

  pressKey(key: Key): Promise<void>;
  pressKeys(keys: Key[]): Promise<void>;
  sendText(text: string): Promise<void>;
  rawWrite(data: Uint8Array | string): Promise<void>;
  clickAt(x: number, y: number): Promise<void>;

  waitFor(options: WaitOptions): Promise<boolean>;
  waitForText(text: string, timeout?: number): Promise<boolean>;
  waitForIdle(idleTime?: number, timeout?: number): Promise<void>;

  resize(cols: number, rows: number): Promise<void>;
  sendSignal(signal: Signal): Promise<void>;
  close(): Promise<void>;
}
```

## Snapshot Format

Simple span-based format (no semantic classification):

```yaml
- row 1:
  - span "htop - ubuntu" [bold] [inverse] [ref=s1] (1,1)
- row 2:
  - span "CPU[" [ref=s2] (1,2)
  - span "||||||||" [fg=green] [ref=s3] (5,2)
  - span "  45.2%]" [ref=s4] (13,2)
- row 5:
  - span "PID" [bold] [ref=s10] (1,5)
  - span "USER" [bold] [ref=s11] (8,5)
- row 6 [inverse]:
  - span "1234" [ref=s14] (1,6)
  - span "root" [ref=s15] (8,6)
- row 24:
  - span "F1" [inverse] [ref=s18] (1,24)
  - span "Help" [ref=s19] (3,24)
```

**Design principles:**
- Spans are short text segments with contiguous styling
- Paragraphs are longer text blocks
- Every span gets a ref (LLM decides what's clickable)
- Position is 1-based `(col, row)`
- Styling: `[bold]`, `[italic]`, `[underline]`, `[inverse]`, `[fg=color]`, `[bg=color]`

## Error Handling

```typescript
interface TuiError {
  code: string;
  message: string;
  details?: Record<string, unknown>;
}

type ErrorCode =
  | "SESSION_NOT_FOUND"
  | "SESSION_CLOSED"
  | "LAUNCH_FAILED"
  | "REF_NOT_FOUND"
  | "TIMEOUT"
  | "INVALID_KEY"
  | "INVALID_COORDINATES"
  | "SCRIPT_ERROR"
  | "SCRIPT_TIMEOUT"
  | "SIGNAL_FAILED"
  | "RESIZE_FAILED";
```

## Dependencies

**tui-driver (library):**
- `portable-pty = "0.8"` - Cross-platform PTY
- `vt100 = "0.15"` - Terminal emulation
- `tokio = "1"` - Async runtime
- `serde`, `serde_json` - Serialization
- `thiserror`, `anyhow` - Error handling
- `uuid` - Session IDs
- `parking_lot` - Fast mutex

**mcp-server (binary):**
- `tui-driver` - Core library
- `rmcp = "0.8"` - Official MCP SDK
- `boa_engine = "0.21"` - JavaScript engine
- `schemars` - JSON Schema for MCP tools
- `clap` - CLI parsing
- `tracing` - Logging

## Project Structure

```
mcp-tui-driver/
├── Cargo.toml
├── tui-driver/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── driver.rs
│       ├── pty.rs
│       ├── snapshot.rs
│       ├── keys.rs
│       ├── mouse.rs
│       └── error.rs
│
└── mcp-server/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── tools.rs
        ├── session.rs
        ├── boa.rs
        └── types.rs
```

## Implementation Milestones

### Milestone 1: "The Viewer"
- Cargo workspace setup
- PTY spawn with portable-pty
- Background reader feeding vt100::Parser
- `text()` - plain text snapshot
- `launch()` and `close()` working
- Basic MCP server with tui_launch, tui_text, tui_close

### Milestone 2: "The Keyboard"
- `pressKey()` - special keys
- `sendText()` - raw text input
- `pressKeys()` - key sequences
- Key to ANSI escape sequence mapping
- `waitForIdle()` and `waitForText()`

### Milestone 3: "The Snapshot"
- Span extraction from vt100 cells
- YAML snapshot format with refs
- `snapshot()` returning structured data
- Styling attributes
- `screenshot()` - render to PNG

### Milestone 4: "The Clicker"
- SGR mouse mode sequences
- `click()`, `clickAt()`, `doubleClick()`, `rightClick()`
- Coordinate mapping (ref to x,y)

### Milestone 5: "The Scripter"
- Boa engine integration
- Inject `tui` object with session-bound API
- `runCode()` execution with timeout
- Error capture and formatting

### Milestone 6: "Production Ready"
- Multi-session support
- `resize()`, `sendSignal()`, `listSessions()`
- Comprehensive error handling
- Logging and tracing
- README and usage docs

## References

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [rmcp (Official Rust SDK)](https://github.com/modelcontextprotocol/rust-sdk)
- [Playwright MCP](https://github.com/microsoft/playwright-mcp)
- [Boa JavaScript Engine](https://github.com/boa-dev/boa)
- [portable-pty](https://docs.rs/portable-pty/latest/portable_pty/)
- [vt100](https://docs.rs/vt100/latest/vt100/)
