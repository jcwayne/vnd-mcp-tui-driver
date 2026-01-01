//! TUI Driver - Headless terminal automation library

pub mod driver;
pub mod error;

pub use driver::{LaunchOptions, TuiDriver};
pub use error::{Result, TuiError};
