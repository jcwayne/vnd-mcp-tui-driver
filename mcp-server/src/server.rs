//! TUI MCP Server implementation using rmcp
//!
//! This module provides the MCP server implementation for TUI automation
//! using the rmcp library.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use boa_engine::Context as JsContext;
use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters, ServerHandler},
    model::{
        CallToolResult, Content, Implementation, ListToolsResult, ServerCapabilities, ServerInfo,
        ToolsCapability,
    },
    service::{RequestContext, RoleServer},
    tool, tool_router, ErrorData as McpError,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use tui_driver::{Key, LaunchOptions, Signal, TuiDriver};

use crate::tools::{
    BufferResult, ClickAtParams, ClickParams, ConsoleLogEntry, GetInputParams, GetOutputParams,
    LaunchParams, LaunchResult, ListSessionsResult, PressKeyParams, PressKeysParams, ResizeParams,
    RunCodeParams, RunCodeResult, ScreenshotResult, ScrollbackResult, SendTextParams,
    SessionInfoResult, SessionParams, SignalParams, SnapshotResult, SuccessResult, TextResult,
    WaitForIdleParams, WaitForTextParams, WaitResult,
};

// =========================================================================
// Session State Types
// =========================================================================

/// Entry for console output captured from JavaScript execution.
///
/// This struct stores messages logged via console.log/warn/error in JS code.
#[derive(Debug, Clone, Serialize)]
pub struct ConsoleEntry {
    /// Log level: "log", "warn", "error", "info", or "debug"
    pub level: String,
    /// The logged message content
    pub message: String,
}

/// State for a single TUI session.
///
/// Wraps the TuiDriver along with optional persistent JavaScript context
/// and collected console logs. The JS context is lazily initialized on
/// the first `tui_run_code` call and reused for subsequent calls,
/// allowing variables to persist across executions.
///
/// # Safety
///
/// `boa_engine::Context` is `!Send + !Sync`, but this is safe because:
/// 1. Access to SessionState is serialized through a Mutex
/// 2. The JS context is only accessed from the MCP server's async handlers
/// 3. All access is done while holding the Mutex lock
pub struct SessionState {
    /// The underlying TUI driver instance
    driver: TuiDriver,
    /// Lazily-initialized JavaScript context for run_code
    /// Created on first use, reused for variable persistence
    js_context: Option<JsContext>,
    /// Console output collected from JavaScript execution
    console_logs: Vec<ConsoleEntry>,
}

// SAFETY: SessionState contains a `boa_engine::Context` which is `!Send + !Sync`.
// This is safe because:
// 1. All access to SessionState is serialized through tokio::sync::Mutex
// 2. The JS context is only accessed in tui_run_code, which executes synchronously
// 3. While some handlers await while holding the lock (wait_for_text, wait_for_idle, close),
//    they only access the driver field, never the js_context
// 4. No concurrent access to the JS context is possible due to mutex serialization
unsafe impl Send for SessionState {}
unsafe impl Sync for SessionState {}

impl SessionState {
    /// Create a new SessionState wrapping the given TuiDriver.
    ///
    /// The JS context starts as None and will be created on demand
    /// when `tui_run_code` is first called.
    pub fn new(driver: TuiDriver) -> Self {
        Self {
            driver,
            js_context: None,
            console_logs: Vec::new(),
        }
    }

    /// Get an immutable reference to the TuiDriver.
    pub fn driver(&self) -> &TuiDriver {
        &self.driver
    }

    /// Get a mutable reference to the TuiDriver.
    pub fn driver_mut(&mut self) -> &mut TuiDriver {
        &mut self.driver
    }

    /// Get a reference to the JavaScript context if initialized
    pub fn js_context(&self) -> Option<&JsContext> {
        self.js_context.as_ref()
    }

    /// Get a mutable reference to the JavaScript context if initialized
    pub fn js_context_mut(&mut self) -> Option<&mut JsContext> {
        self.js_context.as_mut()
    }

    /// Set the JavaScript context
    pub fn set_js_context(&mut self, context: JsContext) {
        self.js_context = Some(context);
    }

    /// Get collected console logs
    pub fn console_logs(&self) -> &[ConsoleEntry] {
        &self.console_logs
    }

    /// Add a console log entry
    pub fn add_console_log(&mut self, entry: ConsoleEntry) {
        self.console_logs.push(entry);
    }

    /// Take and clear console logs (for returning with run_code result)
    pub fn take_console_logs(&mut self) -> Vec<ConsoleEntry> {
        std::mem::take(&mut self.console_logs)
    }
}

// =========================================================================
// Closed Session Data
// =========================================================================

/// Directory for storing closed session debug data
const CLOSED_SESSIONS_DIR: &str = "/tmp/tui-driver-sessions";

/// Data saved when a session is closed (for post-mortem debugging)
#[derive(Debug, Serialize, Deserialize)]
struct ClosedSessionData {
    session_id: String,
    command: String,
    input_buffer: String,
    output_buffer: String,
    scrollback_lines: usize,
    closed_at: u64,
}

/// Get the path for a closed session's data file
fn closed_session_path(session_id: &str) -> PathBuf {
    PathBuf::from(CLOSED_SESSIONS_DIR).join(format!("{}.json", session_id))
}

/// Save closed session data to disk
fn save_closed_session(driver: &TuiDriver) -> anyhow::Result<()> {
    // Ensure directory exists
    fs::create_dir_all(CLOSED_SESSIONS_DIR)?;

    let info = driver.info();
    let data = ClosedSessionData {
        session_id: info.session_id.clone(),
        command: info.command,
        input_buffer: driver.get_input_buffer(10000),
        output_buffer: driver.get_output_buffer(10000),
        scrollback_lines: driver.get_scrollback(),
        closed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    let path = closed_session_path(&info.session_id);
    let json = serde_json::to_string_pretty(&data)?;
    fs::write(path, json)?;

    Ok(())
}

/// Load closed session data from disk
fn load_closed_session(session_id: &str) -> Option<ClosedSessionData> {
    let path = closed_session_path(session_id);
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
    } else {
        None
    }
}

// =========================================================================
// TypeScript Interface Generation
// =========================================================================

/// Generate TypeScript interface definitions for the tui_run_code JavaScript API.
///
/// This provides LLMs with type information before using tui_run_code,
/// helping them understand the available methods and their signatures.
fn generate_typescript_interface() -> String {
    r#"// TypeScript interface for tui_run_code JavaScript API

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
  sendSignal(signal: "SIGINT" | "SIGTERM" | "SIGKILL" | "SIGHUP" | "SIGQUIT"): void;

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
  width: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  inverse?: boolean;
  strikethrough?: boolean;
  underline_style?: "single" | "double" | "curly" | "dotted" | "dashed";
  blink?: "slow" | "rapid";
  fg?: string;
  bg?: string;
  link?: string;
  image?: string;
  image_size?: string;
}

interface Console {
  log(...args: any[]): void;
  info(...args: any[]): void;
  warn(...args: any[]): void;
  error(...args: any[]): void;
  debug(...args: any[]): void;
}

declare const tui: Tui;
declare const console: Console;
"#
    .to_string()
}

/// TUI MCP Server
///
/// This struct implements the MCP server for TUI automation.
/// It manages multiple TUI sessions and exposes them as MCP tools.
#[derive(Clone)]
pub struct TuiServer {
    /// Active TUI sessions indexed by session ID
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    /// Tool router for handling tool calls
    tool_router: ToolRouter<Self>,
}

impl TuiServer {
    /// Create a new TuiServer instance
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for TuiServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl TuiServer {
    // =========================================================================
    // Session Management Tools
    // =========================================================================

    /// Launch a new TUI application session
    #[tool(description = "Launch a new TUI application session")]
    async fn tui_launch(
        &self,
        Parameters(params): Parameters<LaunchParams>,
    ) -> Result<CallToolResult, McpError> {
        let options = LaunchOptions::new(&params.command)
            .args(params.args)
            .size(params.cols, params.rows);

        match TuiDriver::launch(options).await {
            Ok(driver) => {
                let session_id = driver.session_id().to_string();
                let mut sessions = self.sessions.lock().await;
                sessions.insert(session_id.clone(), SessionState::new(driver));

                let result = LaunchResult { session_id };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error launching session: {}",
                e
            ))])),
        }
    }

    /// Close a TUI session
    #[tool(description = "Close a TUI session")]
    async fn tui_close(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut sessions = self.sessions.lock().await;
        match sessions.remove(&params.session_id) {
            Some(session) => {
                // Save debug buffers to disk before closing
                if let Err(e) = save_closed_session(session.driver()) {
                    error!("Error saving closed session data: {}", e);
                }

                // Close the driver
                if let Err(e) = session.driver().close().await {
                    error!("Error closing session: {}", e);
                }

                let result = SuccessResult { success: true };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// List all active TUI sessions
    #[tool(description = "List all active TUI sessions")]
    async fn tui_list_sessions(&self) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();

        let result = ListSessionsResult {
            sessions: session_ids,
        };
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string(&result).unwrap(),
        )]))
    }

    /// Get information about a TUI session
    #[tool(description = "Get information about a TUI session")]
    async fn tui_get_session(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let info = session.driver().info();
                let result = SessionInfoResult {
                    session_id: info.session_id,
                    command: info.command,
                    cols: info.cols,
                    rows: info.rows,
                    running: info.running,
                };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Display Tools
    // =========================================================================

    /// Get the current text content of a TUI session
    #[tool(description = "Get the current text content of a TUI session")]
    async fn tui_text(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let text = session.driver().text();
                let result = TextResult { text };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Get accessibility-style snapshot with element references
    #[tool(description = "Get accessibility-style snapshot with element references")]
    async fn tui_snapshot(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let snapshot = session.driver().snapshot();
                let yaml = snapshot.yaml.clone().unwrap_or_default();
                let span_count = snapshot.span_count();

                let result = SnapshotResult { yaml, span_count };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Take a PNG screenshot of the terminal
    #[tool(description = "Take a PNG screenshot of the terminal")]
    async fn tui_screenshot(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let screenshot = session.driver().screenshot();

                let result = ScreenshotResult {
                    data: screenshot.data,
                    format: screenshot.format,
                    width: screenshot.width,
                    height: screenshot.height,
                };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Input Tools
    // =========================================================================

    /// Press a single key in the TUI session
    #[tool(
        description = "Press a single key in the TUI session. Supports special keys (Enter, Tab, Escape, etc.), arrow keys, function keys, and modifier combinations (Ctrl+c, Alt+x)."
    )]
    async fn tui_press_key(
        &self,
        Parameters(params): Parameters<PressKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse the key string
        let key = match Key::parse(&params.key) {
            Ok(k) => k,
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Invalid key: {}",
                    e
                ))]));
            }
        };

        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().press_key(&key) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error pressing key: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Press multiple keys in sequence in the TUI session
    #[tool(description = "Press multiple keys in sequence in the TUI session")]
    async fn tui_press_keys(
        &self,
        Parameters(params): Parameters<PressKeysParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse all keys first
        let mut keys = Vec::new();
        for key_str in &params.keys {
            match Key::parse(key_str) {
                Ok(k) => keys.push(k),
                Err(e) => {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "Invalid key '{}': {}",
                        key_str, e
                    ))]));
                }
            }
        }

        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().press_keys(&keys) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error pressing keys: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Send raw text to the TUI session (useful for typing strings)
    #[tool(description = "Send raw text to the TUI session (useful for typing strings)")]
    async fn tui_send_text(
        &self,
        Parameters(params): Parameters<SendTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().send_text(&params.text) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error sending text: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Mouse Tools
    // =========================================================================

    /// Click on an element by reference ID from the snapshot
    #[tool(description = "Click on an element by reference ID from the snapshot")]
    async fn tui_click(
        &self,
        Parameters(params): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().click(&params.ref_id) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error clicking element: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Click at specific coordinates in the terminal
    #[tool(description = "Click at specific coordinates in the terminal")]
    async fn tui_click_at(
        &self,
        Parameters(params): Parameters<ClickAtParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().click_at(params.x, params.y) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error clicking at coordinates: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Double-click on an element by reference ID from the snapshot
    #[tool(description = "Double-click on an element by reference ID from the snapshot")]
    async fn tui_double_click(
        &self,
        Parameters(params): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().double_click(&params.ref_id) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error double-clicking element: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Right-click on an element by reference ID from the snapshot
    #[tool(description = "Right-click on an element by reference ID from the snapshot")]
    async fn tui_right_click(
        &self,
        Parameters(params): Parameters<ClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().right_click(&params.ref_id) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error right-clicking element: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Wait Tools
    // =========================================================================

    /// Wait for specific text to appear on the screen
    #[tool(description = "Wait for specific text to appear on the screen")]
    async fn tui_wait_for_text(
        &self,
        Parameters(params): Parameters<WaitForTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                match session
                    .driver()
                    .wait_for_text(&params.text, params.timeout_ms)
                    .await
                {
                    Ok(found) => {
                        let result = WaitResult { found };
                        Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string(&result).unwrap(),
                        )]))
                    }
                    Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                        "Error waiting for text: {}",
                        e
                    ))])),
                }
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Wait for the screen to stop changing (become idle)
    #[tool(description = "Wait for the screen to stop changing (become idle)")]
    async fn tui_wait_for_idle(
        &self,
        Parameters(params): Parameters<WaitForIdleParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                match session
                    .driver()
                    .wait_for_idle(params.idle_ms, params.timeout_ms)
                    .await
                {
                    Ok(()) => {
                        let result = SuccessResult { success: true };
                        Ok(CallToolResult::success(vec![Content::text(
                            serde_json::to_string(&result).unwrap(),
                        )]))
                    }
                    Err(e) => {
                        // Timeout is not really an error, just means it didn't become idle
                        Ok(CallToolResult::error(vec![Content::text(format!(
                            "Timeout waiting for idle: {}",
                            e
                        ))]))
                    }
                }
            }
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Control Tools
    // =========================================================================

    /// Resize the terminal window
    #[tool(description = "Resize the terminal window")]
    async fn tui_resize(
        &self,
        Parameters(params): Parameters<ResizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().resize(params.cols, params.rows) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error resizing terminal: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    /// Send a signal to the TUI process (SIGINT, SIGTERM, SIGKILL, SIGHUP, SIGQUIT)
    #[tool(
        description = "Send a signal to the TUI process (SIGINT, SIGTERM, SIGKILL, SIGHUP, SIGQUIT)"
    )]
    async fn tui_send_signal(
        &self,
        Parameters(params): Parameters<SignalParams>,
    ) -> Result<CallToolResult, McpError> {
        // Parse signal name to Signal enum
        let signal = match params.signal.to_uppercase().as_str() {
            "SIGINT" | "INT" => Signal::Int,
            "SIGTERM" | "TERM" => Signal::Term,
            "SIGKILL" | "KILL" => Signal::Kill,
            "SIGHUP" | "HUP" => Signal::Hup,
            "SIGQUIT" | "QUIT" => Signal::Quit,
            _ => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Unknown signal: {}. Supported signals: SIGINT, SIGTERM, SIGKILL, SIGHUP, SIGQUIT",
                    params.signal
                ))]));
            }
        };

        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match session.driver().send_signal(signal) {
                Ok(()) => {
                    let result = SuccessResult { success: true };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error sending signal: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Script Tools
    // =========================================================================

    /// Get TypeScript interface definitions for tui_run_code
    #[tool(
        description = "Get TypeScript interface definitions for tui_run_code. Call this before using tui_run_code to understand the available API."
    )]
    async fn tui_get_code_interface(&self) -> Result<CallToolResult, McpError> {
        let interface = generate_typescript_interface();
        Ok(CallToolResult::success(vec![Content::text(interface)]))
    }

    /// Execute JavaScript code with tui object for complex automation
    #[tool(
        description = "Execute JavaScript code with tui object for complex automation. Available: tui.text(), tui.sendText(text), tui.pressKey(key), tui.clickAt(x,y), tui.snapshot()"
    )]
    async fn tui_run_code(
        &self,
        Parameters(params): Parameters<RunCodeParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => match crate::boa::execute_script(session.driver(), &params.code) {
                Ok((result_str, logs)) => {
                    let log_entries: Vec<ConsoleLogEntry> = logs
                        .into_iter()
                        .map(|e| ConsoleLogEntry {
                            level: e.level,
                            message: e.message,
                        })
                        .collect();
                    let result = RunCodeResult {
                        result: result_str,
                        logs: log_entries,
                    };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                }
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error executing JavaScript: {}",
                    e
                ))])),
            },
            None => Ok(CallToolResult::error(vec![Content::text(format!(
                "Session not found: {}",
                params.session_id
            ))])),
        }
    }

    // =========================================================================
    // Debug Tools
    // =========================================================================

    /// Get raw input sent to the process (escape sequences included)
    #[tool(
        description = "Get raw input sent to the process (escape sequences included). Useful for debugging what was sent to the terminal."
    )]
    async fn tui_get_input(
        &self,
        Parameters(params): Parameters<GetInputParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let content = session.driver().get_input_buffer(params.chars);
                let result = BufferResult {
                    length: content.len(),
                    content,
                };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => {
                // Check for closed session on disk
                if let Some(closed) = load_closed_session(&params.session_id) {
                    let content = if params.chars >= closed.input_buffer.len() {
                        closed.input_buffer
                    } else {
                        closed
                            .input_buffer
                            .chars()
                            .rev()
                            .take(params.chars)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect()
                    };
                    let result = BufferResult {
                        length: content.len(),
                        content,
                    };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                } else {
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "Session not found: {}",
                        params.session_id
                    ))]))
                }
            }
        }
    }

    /// Get raw PTY output (escape sequences included)
    #[tool(
        description = "Get raw PTY output (escape sequences included). Useful for debugging terminal output."
    )]
    async fn tui_get_output(
        &self,
        Parameters(params): Parameters<GetOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let content = session.driver().get_output_buffer(params.chars);
                let result = BufferResult {
                    length: content.len(),
                    content,
                };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => {
                // Check for closed session on disk
                if let Some(closed) = load_closed_session(&params.session_id) {
                    let content = if params.chars >= closed.output_buffer.len() {
                        closed.output_buffer
                    } else {
                        closed
                            .output_buffer
                            .chars()
                            .rev()
                            .take(params.chars)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect()
                    };
                    let result = BufferResult {
                        length: content.len(),
                        content,
                    };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                } else {
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "Session not found: {}",
                        params.session_id
                    ))]))
                }
            }
        }
    }

    /// Get the number of lines that have scrolled off the visible screen
    #[tool(description = "Get the number of lines that have scrolled off the visible screen.")]
    async fn tui_get_scrollback(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(session) => {
                let lines = session.driver().get_scrollback();
                let result = ScrollbackResult { lines };
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string(&result).unwrap(),
                )]))
            }
            None => {
                // Check for closed session on disk
                if let Some(closed) = load_closed_session(&params.session_id) {
                    let result = ScrollbackResult {
                        lines: closed.scrollback_lines,
                    };
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string(&result).unwrap(),
                    )]))
                } else {
                    Ok(CallToolResult::error(vec![Content::text(format!(
                        "Session not found: {}",
                        params.session_id
                    ))]))
                }
            }
        }
    }
}

impl ServerHandler for TuiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mcp-tui-driver".to_string(),
                title: Some("TUI Driver MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "MCP server for TUI automation. Use tui_launch to start a session.".to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move { Ok(ListToolsResult::with_all_items(self.tool_router.list_all())) }
    }

    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let ctx =
                rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
            self.tool_router.call(ctx).await
        }
    }
}
