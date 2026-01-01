//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;
pub mod keys;
pub mod mouse;
pub mod snapshot;

pub use driver::{LaunchOptions, Signal, TuiDriver};
pub use error::{Result, TuiError};
pub use keys::Key;
pub use mouse::MouseButton;
pub use snapshot::{build_snapshot, render_screenshot, Row, Screenshot, Snapshot, Span};
