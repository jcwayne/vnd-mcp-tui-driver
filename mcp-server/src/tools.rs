//! MCP tool definitions for TUI automation

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Parameters for the tui_launch tool
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LaunchParams {
    /// Command to execute
    pub command: String,
    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,
    /// Terminal width in columns
    #[serde(default = "default_cols")]
    pub cols: u16,
    /// Terminal height in rows
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 {
    80
}

fn default_rows() -> u16 {
    24
}

/// Parameters for tools that operate on an existing session
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SessionParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
}

/// Result of launching a TUI session
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LaunchResult {
    pub session_id: String,
}

/// Result of getting text from a session
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TextResult {
    pub text: String,
}

/// Result of closing a session
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CloseResult {
    pub success: bool,
}

/// Parameters for pressing a single key
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PressKeyParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Key to press (e.g., "Enter", "Tab", "Ctrl+c", "a")
    pub key: String,
}

/// Parameters for pressing multiple keys
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PressKeysParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Keys to press in sequence
    pub keys: Vec<String>,
}

/// Parameters for sending raw text
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SendTextParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Text to send to the terminal
    pub text: String,
}

/// Parameters for waiting for text to appear
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitForTextParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Text to wait for
    pub text: String,
    /// Timeout in milliseconds (default: 5000)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// Parameters for waiting for screen to become idle
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitForIdleParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// How long screen must be stable to be considered idle (default: 100ms)
    #[serde(default = "default_idle_ms")]
    pub idle_ms: u64,
    /// Timeout in milliseconds (default: 5000)
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    5000
}

fn default_idle_ms() -> u64 {
    100
}

/// Result of a wait operation
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct WaitResult {
    /// Whether the condition was met before timeout
    pub found: bool,
}

/// Result indicating success
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SuccessResult {
    /// Whether the operation succeeded
    pub success: bool,
}

/// Result of getting an accessibility-style snapshot
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SnapshotResult {
    /// YAML representation of the snapshot
    pub yaml: String,
    /// Number of spans in the snapshot
    pub span_count: usize,
}

/// Result of taking a screenshot
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScreenshotResult {
    /// Base64-encoded PNG image data
    pub data: String,
    /// Image format (always "png")
    pub format: String,
    /// Image width in pixels
    pub width: u32,
    /// Image height in pixels
    pub height: u32,
}

/// Parameters for clicking on an element by reference ID
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClickParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Element reference ID from snapshot
    pub ref_id: String,
}

/// Parameters for clicking at specific coordinates
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ClickAtParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// X coordinate (1-based column)
    pub x: u16,
    /// Y coordinate (1-based row)
    pub y: u16,
}

/// Parameters for running JavaScript code
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunCodeParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// JavaScript code to execute
    pub code: String,
}

/// A console log entry from JavaScript execution.
///
/// Represents output from console.log/warn/error/info/debug calls.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConsoleLogEntry {
    /// Log level: "log", "warn", "error", "info", or "debug"
    pub level: String,
    /// The logged message content
    pub message: String,
}

/// Result of running JavaScript code
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct RunCodeResult {
    /// Result of the script execution
    pub result: String,
    /// Console output captured during execution
    pub logs: Vec<ConsoleLogEntry>,
}

/// Parameters for resizing the terminal
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ResizeParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// New terminal width in columns
    pub cols: u16,
    /// New terminal height in rows
    pub rows: u16,
}

/// Parameters for sending a signal to the process
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SignalParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Signal to send (SIGINT, SIGTERM, SIGKILL, SIGHUP, SIGQUIT)
    pub signal: String,
}

/// Result of listing sessions
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ListSessionsResult {
    /// List of active session IDs
    pub sessions: Vec<String>,
}

/// Result of getting session info
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SessionInfoResult {
    /// Session identifier
    pub session_id: String,
    /// Command that was launched
    pub command: String,
    /// Terminal width in columns
    pub cols: u16,
    /// Terminal height in rows
    pub rows: u16,
    /// Whether the process is still running
    pub running: bool,
}

/// Parameters for getting input buffer (raw escape sequences sent to process)
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetInputParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Maximum characters to return (default: 10000)
    #[serde(default = "default_buffer_chars")]
    pub chars: usize,
}

/// Parameters for getting output buffer (raw PTY output)
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct GetOutputParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Maximum characters to return (default: 10000)
    #[serde(default = "default_buffer_chars")]
    pub chars: usize,
}

fn default_buffer_chars() -> usize {
    10000
}

/// Result of getting a debug buffer
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct BufferResult {
    /// Buffer content
    pub content: String,
    /// Number of characters in the result
    pub length: usize,
}

/// Result of getting scrollback info
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ScrollbackResult {
    /// Number of lines that have scrolled off screen
    pub lines: usize,
}
