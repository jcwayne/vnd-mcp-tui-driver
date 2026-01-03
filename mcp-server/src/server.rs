//! TUI MCP Server implementation using rmcp
//!
//! This module provides the MCP server implementation for TUI automation
//! using the rmcp library.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use rmcp::{
    model::{
        CallToolResult, Content, Implementation, ListToolsResult, ServerCapabilities, ServerInfo,
        ToolsCapability,
    },
    handler::server::{tool::ToolCallContext, ServerHandler},
    service::{RequestContext, RoleServer},
    tool, tool_router,
    handler::server::tool::ToolRouter,
    ErrorData as McpError,
};

use tui_driver::TuiDriver;

/// TUI MCP Server
///
/// This struct implements the MCP server for TUI automation.
/// It manages multiple TUI sessions and exposes them as MCP tools.
#[derive(Clone)]
pub struct TuiServer {
    /// Active TUI sessions indexed by session ID
    sessions: Arc<Mutex<HashMap<String, TuiDriver>>>,
    /// Tool router for handling tool calls
    tool_router: ToolRouter<Self>,
}

impl TuiServer {
    /// Create a new TuiServer instance
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tool_router: Self::tool_router(),
        }
    }
}

impl Default for TuiServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl TuiServer {
    /// Placeholder tool for initial setup - will be replaced with actual TUI tools
    #[tool(description = "List all active TUI sessions")]
    async fn tui_list_sessions(&self) -> Result<CallToolResult, McpError> {
        let sessions = self.sessions.lock().await;
        let session_ids: Vec<String> = sessions.keys().cloned().collect();
        let json = serde_json::json!({ "sessions": session_ids });
        Ok(CallToolResult::success(vec![Content::text(
            json.to_string(),
        )]))
    }
}

impl ServerHandler for TuiServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: Default::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mcp-tui-driver".to_string(),
                title: Some("TUI Driver MCP Server".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "MCP server for TUI automation. Use tui_launch to start a session.".to_string(),
            ),
        }
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        async move { Ok(ListToolsResult::with_all_items(self.tool_router.list_all())) }
    }

    fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, McpError>> + Send + '_ {
        async move {
            let ctx = ToolCallContext::new(self, request, context);
            self.tool_router.call(ctx).await
        }
    }
}
