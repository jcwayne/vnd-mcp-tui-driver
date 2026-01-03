//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;
pub mod keys;
pub mod mouse;
pub mod snapshot;
pub mod terminal;

pub use driver::{LaunchOptions, SessionInfo, Signal, TuiDriver};
pub use error::{Result, TuiError};
pub use keys::Key;
pub use mouse::MouseButton;
pub use snapshot::{
    build_snapshot, build_snapshot_from_wezterm, render_screenshot, render_screenshot_from_wezterm,
    Row, Screenshot, Snapshot, Span,
};
pub use terminal::TuiTerminal;
