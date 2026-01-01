//! MCP TUI Driver - MCP server for TUI automation
//!
//! This server exposes TUI automation capabilities via the Model Context Protocol (MCP).
//! It communicates over stdin/stdout using JSON-RPC 2.0.

mod tools;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use tui_driver::{LaunchOptions, TuiDriver};

use crate::tools::{CloseResult, LaunchParams, LaunchResult, SessionParams, TextResult};

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

/// Session manager holding all active TUI sessions
struct SessionManager {
    sessions: HashMap<String, TuiDriver>,
}

impl SessionManager {
    fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    fn insert(&mut self, session_id: String, driver: TuiDriver) {
        self.sessions.insert(session_id, driver);
    }

    fn get(&self, session_id: &str) -> Option<&TuiDriver> {
        self.sessions.get(session_id)
    }

    fn remove(&mut self, session_id: &str) -> Option<TuiDriver> {
        self.sessions.remove(session_id)
    }
}

/// MCP Server implementation
struct McpServer {
    sessions: Arc<Mutex<SessionManager>>,
}

impl McpServer {
    fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(SessionManager::new())),
        }
    }

    /// Handle an incoming JSON-RPC request
    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let id = request.id.clone().unwrap_or(Value::Null);

        debug!("Handling request: method={}", request.method);

        match request.method.as_str() {
            "initialize" => self.handle_initialize(id, request.params).await,
            "notifications/initialized" => {
                // This is a notification, no response needed
                // Return a success response anyway since we need to respond
                JsonRpcResponse::success(id, json!({}))
            }
            "tools/list" => self.handle_tools_list(id).await,
            "tools/call" => self.handle_tools_call(id, request.params).await,
            _ => {
                JsonRpcResponse::error(id, -32601, format!("Method not found: {}", request.method))
            }
        }
    }

    /// Handle the initialize method
    async fn handle_initialize(&self, id: Value, _params: Value) -> JsonRpcResponse {
        info!("MCP server initializing");

        let result = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "mcp-tui-driver",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        JsonRpcResponse::success(id, result)
    }

    /// Handle the tools/list method
    async fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        let tools = json!({
            "tools": [
                {
                    "name": "tui_launch",
                    "description": "Launch a new TUI application session",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "Command to execute"
                            },
                            "args": {
                                "type": "array",
                                "items": { "type": "string" },
                                "description": "Command arguments",
                                "default": []
                            },
                            "cols": {
                                "type": "integer",
                                "description": "Terminal width in columns",
                                "default": 80
                            },
                            "rows": {
                                "type": "integer",
                                "description": "Terminal height in rows",
                                "default": 24
                            }
                        },
                        "required": ["command"]
                    }
                },
                {
                    "name": "tui_text",
                    "description": "Get the current text content of a TUI session",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session identifier returned by tui_launch"
                            }
                        },
                        "required": ["session_id"]
                    }
                },
                {
                    "name": "tui_close",
                    "description": "Close a TUI session",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session identifier returned by tui_launch"
                            }
                        },
                        "required": ["session_id"]
                    }
                }
            ]
        });

        JsonRpcResponse::success(id, tools)
    }

    /// Handle the tools/call method
    async fn handle_tools_call(&self, id: Value, params: Value) -> JsonRpcResponse {
        let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!("Calling tool: {} with args: {:?}", tool_name, arguments);

        match tool_name {
            "tui_launch" => self.tool_launch(id, arguments).await,
            "tui_text" => self.tool_text(id, arguments).await,
            "tui_close" => self.tool_close(id, arguments).await,
            _ => JsonRpcResponse::error(id, -32602, format!("Unknown tool: {}", tool_name)),
        }
    }

    /// Handle tui_launch tool
    async fn tool_launch(&self, id: Value, arguments: Value) -> JsonRpcResponse {
        let params: LaunchParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
            }
        };

        let options = LaunchOptions::new(&params.command)
            .args(params.args)
            .size(params.cols, params.rows);

        match TuiDriver::launch(options).await {
            Ok(driver) => {
                let session_id = driver.session_id().to_string();
                let mut sessions = self.sessions.lock().await;
                sessions.insert(session_id.clone(), driver);

                let result = LaunchResult { session_id };
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string(&result).unwrap()
                        }
                    ]
                });
                JsonRpcResponse::success(id, content)
            }
            Err(e) => {
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Error launching session: {}", e)
                        }
                    ],
                    "isError": true
                });
                JsonRpcResponse::success(id, content)
            }
        }
    }

    /// Handle tui_text tool
    async fn tool_text(&self, id: Value, arguments: Value) -> JsonRpcResponse {
        let params: SessionParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
            }
        };

        let sessions = self.sessions.lock().await;
        match sessions.get(&params.session_id) {
            Some(driver) => {
                let text = driver.text();
                let result = TextResult { text };
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string(&result).unwrap()
                        }
                    ]
                });
                JsonRpcResponse::success(id, content)
            }
            None => {
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Session not found: {}", params.session_id)
                        }
                    ],
                    "isError": true
                });
                JsonRpcResponse::success(id, content)
            }
        }
    }

    /// Handle tui_close tool
    async fn tool_close(&self, id: Value, arguments: Value) -> JsonRpcResponse {
        let params: SessionParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid parameters: {}", e));
            }
        };

        let mut sessions = self.sessions.lock().await;
        match sessions.remove(&params.session_id) {
            Some(driver) => {
                // Close the driver
                if let Err(e) = driver.close().await {
                    error!("Error closing session: {}", e);
                }

                let result = CloseResult { success: true };
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string(&result).unwrap()
                        }
                    ]
                });
                JsonRpcResponse::success(id, content)
            }
            None => {
                let content = json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Session not found: {}", params.session_id)
                        }
                    ],
                    "isError": true
                });
                JsonRpcResponse::success(id, content)
            }
        }
    }
}

/// Read a single JSON-RPC message from stdin
fn read_message(reader: &mut impl BufRead) -> Result<Option<String>> {
    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line)?;

    if bytes_read == 0 {
        return Ok(None);
    }

    Ok(Some(line))
}

/// Write a JSON-RPC response to stdout
fn write_response(response: &JsonRpcResponse) -> Result<()> {
    let json = serde_json::to_string(response)?;
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{}", json)?;
    stdout.flush()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to stderr (stdout is for MCP messages)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mcp_tui_driver=debug".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("MCP TUI Driver starting");

    let server = McpServer::new();
    let mut stdin = std::io::stdin().lock();

    loop {
        match read_message(&mut stdin) {
            Ok(Some(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                debug!("Received: {}", line);

                match serde_json::from_str::<JsonRpcRequest>(line) {
                    Ok(request) => {
                        // Check if this is a notification (no id)
                        let is_notification = request.id.is_none();

                        let response = server.handle_request(request).await;

                        // Only send response if it's not a notification
                        if !is_notification {
                            if let Err(e) = write_response(&response) {
                                error!("Failed to write response: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        let response = JsonRpcResponse::error(
                            Value::Null,
                            -32700,
                            format!("Parse error: {}", e),
                        );
                        if let Err(e) = write_response(&response) {
                            error!("Failed to write error response: {}", e);
                        }
                    }
                }
            }
            Ok(None) => {
                // EOF - stdin closed
                info!("stdin closed, exiting");
                break;
            }
            Err(e) => {
                error!("Error reading from stdin: {}", e);
                break;
            }
        }
    }

    info!("MCP TUI Driver shutting down");
    Ok(())
}
