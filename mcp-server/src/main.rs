//! MCP TUI Driver - MCP server for TUI automation
//!
//! This server exposes TUI automation capabilities via the Model Context Protocol (MCP).
//! It supports both stdio (default) and SSE transports.

use mcp_tui_driver::TuiServer;

use anyhow::Result;
use clap::Parser;
use rmcp::ServiceExt;
use tracing::debug;

/// Playwright MCP for TUI apps
#[derive(Parser)]
#[command(name = "mcp-tui-driver")]
#[command(about = "Playwright MCP for TUI apps")]
#[command(version)]
struct Cli {
    /// Run in SSE mode instead of stdio
    #[arg(long)]
    sse: bool,

    /// Port for SSE server
    #[arg(long, default_value = "8080")]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is for MCP messages)
    // Default to warn level; set RUST_LOG=mcp_tui_driver=debug for verbose logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mcp_tui_driver=warn".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let server = TuiServer::new();

    if cli.sse {
        // SSE transport not implemented yet
        anyhow::bail!("SSE transport not yet implemented");
    } else {
        // Use stdio transport
        debug!("MCP TUI Driver starting with stdio transport");
        let transport = rmcp::transport::stdio();
        let service = server.serve(transport).await?;

        // Wait for service to complete
        service.waiting().await?;
        debug!("MCP TUI Driver shutting down");
    }

    Ok(())
}
