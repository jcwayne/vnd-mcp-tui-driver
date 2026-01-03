//! MCP TUI Driver library
//!
//! This crate provides an MCP server for TUI automation.

mod boa;
mod server;
pub mod tools;

pub use server::TuiServer;
