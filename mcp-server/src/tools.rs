//! MCP tool definitions for TUI automation

use serde::{Deserialize, Serialize};

/// Parameters for the tui_launch tool
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
}

/// Result of launching a TUI session
#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchResult {
    pub session_id: String,
}

/// Result of getting text from a session
#[derive(Debug, Serialize, Deserialize)]
pub struct TextResult {
    pub text: String,
}

/// Result of closing a session
#[derive(Debug, Serialize, Deserialize)]
pub struct CloseResult {
    pub success: bool,
}

/// Parameters for pressing a single key
#[derive(Debug, Deserialize)]
pub struct PressKeyParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Key to press (e.g., "Enter", "Tab", "Ctrl+c", "a")
    pub key: String,
}

/// Parameters for pressing multiple keys
#[derive(Debug, Deserialize)]
pub struct PressKeysParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Keys to press in sequence
    pub keys: Vec<String>,
}

/// Parameters for sending raw text
#[derive(Debug, Deserialize)]
pub struct SendTextParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Text to send to the terminal
    pub text: String,
}

/// Parameters for waiting for text to appear
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Serialize)]
pub struct WaitResult {
    /// Whether the condition was met before timeout
    pub found: bool,
}

/// Result indicating success
#[derive(Debug, Serialize)]
pub struct SuccessResult {
    /// Whether the operation succeeded
    pub success: bool,
}

/// Result of getting an accessibility-style snapshot
#[derive(Debug, Serialize)]
pub struct SnapshotResult {
    /// YAML representation of the snapshot
    pub yaml: String,
    /// Number of spans in the snapshot
    pub span_count: usize,
}

/// Result of taking a screenshot
#[derive(Debug, Serialize)]
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
#[derive(Debug, Deserialize)]
pub struct ClickParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// Element reference ID from snapshot
    pub ref_id: String,
}

/// Parameters for clicking at specific coordinates
#[derive(Debug, Deserialize)]
pub struct ClickAtParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// X coordinate (1-based column)
    pub x: u16,
    /// Y coordinate (1-based row)
    pub y: u16,
}

/// Parameters for running JavaScript code
#[derive(Debug, Deserialize)]
pub struct RunCodeParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
    /// JavaScript code to execute
    pub code: String,
}

/// Result of running JavaScript code
#[derive(Debug, Serialize)]
pub struct RunCodeResult {
    /// Result of the script execution
    pub result: String,
}
