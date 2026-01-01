//! Error types for tui-driver

use thiserror::Error;

/// Errors that can occur in tui-driver operations
#[derive(Error, Debug)]
pub enum TuiError {
    #[error("Failed to launch process: {0}")]
    LaunchFailed(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Session already closed")]
    SessionClosed,

    #[error("Timeout waiting for condition")]
    Timeout,

    #[error("Invalid key: {0}")]
    InvalidKey(String),

    #[error("Invalid coordinates: ({x}, {y})")]
    InvalidCoordinates { x: u16, y: u16 },

    #[error("Element reference not found: {0}")]
    RefNotFound(String),

    #[error("PTY error: {0}")]
    PtyError(String),

    #[error("Resize failed: {0}")]
    ResizeFailed(String),

    #[error("Failed to send signal: {0}")]
    SignalFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Result type alias for tui-driver operations
pub type Result<T> = std::result::Result<T, TuiError>;
