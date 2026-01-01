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
