//! Error types for TUI Driver

use thiserror::Error;

/// Error type for TUI Driver operations
#[derive(Debug, Error)]
pub enum TuiError {
    /// Placeholder error variant
    #[error("TUI error: {0}")]
    Generic(String),
}
