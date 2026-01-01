//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;
pub mod keys;
pub mod snapshot;

pub use driver::{LaunchOptions, TuiDriver};
pub use error::{Result, TuiError};
pub use keys::Key;
pub use snapshot::{Row, Snapshot, Span};
