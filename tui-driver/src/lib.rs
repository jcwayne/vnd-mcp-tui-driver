//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;
pub mod keys;

pub use driver::{LaunchOptions, TuiDriver};
pub use error::{Result, TuiError};
pub use keys::Key;
