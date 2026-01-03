//! rmcp integration tests for TUI MCP server
//!
//! These tests verify the MCP server functionality by setting up a proper
//! client-server pair using tokio::io::duplex transport.

use mcp_tui_driver::TuiServer;
use mcp_tui_driver::tools::{LaunchResult, ListSessionsResult, SuccessResult, TextResult};
use rmcp::handler::server::ServerHandler;
use rmcp::model::CallToolRequestParam;
use rmcp::{ClientHandler, ServiceExt};
use serde_json::json;

/// Helper to create arguments from a JSON value
fn make_args(value: serde_json::Value) -> Option<serde_json::Map<String, serde_json::Value>> {
    value.as_object().cloned()
}

/// Dummy client handler for testing
#[derive(Debug, Clone, Default)]
struct DummyClientHandler;

impl ClientHandler for DummyClientHandler {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        rmcp::model::ClientInfo::default()
    }
}

/// Helper to extract text content from a CallToolResult
fn extract_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .unwrap_or_default()
}

/// Helper to check if result is an error
fn is_error(result: &rmcp::model::CallToolResult) -> bool {
    result.is_error.unwrap_or(false)
}

#[tokio::test]
async fn test_server_info() {
    let server = TuiServer::new();
    let info = server.get_info();

    assert_eq!(info.server_info.name, "mcp-tui-driver");
    assert!(info.server_info.title.is_some());
    assert_eq!(
        info.server_info.title.unwrap(),
        "TUI Driver MCP Server"
    );
    assert!(info.capabilities.tools.is_some());
}

#[tokio::test]
async fn test_list_tools() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    let tools = client.list_tools(None).await?;

    // Should have multiple tools
    assert!(!tools.tools.is_empty());

    // Check for expected tool names
    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(tool_names.contains(&"tui_launch".to_string()));
    assert!(tool_names.contains(&"tui_close".to_string()));
    assert!(tool_names.contains(&"tui_list_sessions".to_string()));
    assert!(tool_names.contains(&"tui_text".to_string()));
    assert!(tool_names.contains(&"tui_press_key".to_string()));
    assert!(tool_names.contains(&"tui_send_text".to_string()));
    assert!(tool_names.contains(&"tui_snapshot".to_string()));
    assert!(tool_names.contains(&"tui_screenshot".to_string()));

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_list_sessions_empty() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_list_sessions".into(),
            arguments: None,
        })
        .await?;

    assert!(!is_error(&result));

    let text = extract_text(&result);
    let parsed: ListSessionsResult = serde_json::from_str(&text)?;
    assert!(parsed.sessions.is_empty());

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_launch_and_close() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch echo command
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "echo",
                "args": ["Hello from test"],
                "cols": 80,
                "rows": 24
            })),
        })
        .await?;

    assert!(!is_error(&result), "Launch failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let launch_result: LaunchResult = serde_json::from_str(&text)?;
    assert!(!launch_result.session_id.is_empty());

    let session_id = launch_result.session_id.clone();

    // Verify session is listed
    let list_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_list_sessions".into(),
            arguments: None,
        })
        .await?;

    let list_text = extract_text(&list_result);
    let sessions: ListSessionsResult = serde_json::from_str(&list_text)?;
    assert!(sessions.sessions.contains(&session_id));

    // Give the echo command time to produce output
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Close session
    let close_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    assert!(!is_error(&close_result));

    let close_text = extract_text(&close_result);
    let success: SuccessResult = serde_json::from_str(&close_text)?;
    assert!(success.success);

    // Verify session is no longer listed
    let list_result2 = client
        .call_tool(CallToolRequestParam {
            name: "tui_list_sessions".into(),
            arguments: None,
        })
        .await?;

    let list_text2 = extract_text(&list_result2);
    let sessions2: ListSessionsResult = serde_json::from_str(&list_text2)?;
    assert!(!sessions2.sessions.contains(&session_id));

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_invalid_session() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Try to get text from non-existent session
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_text".into(),
            arguments: make_args(json!({
                "session_id": "nonexistent-session-id"
            })),
        })
        .await?;

    assert!(is_error(&result));

    let text = extract_text(&result);
    assert!(text.contains("Session not found"));

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_get_text() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch echo command
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "echo",
                "args": ["UNIQUE_TEST_STRING"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    // Wait for echo to produce output
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Get text
    let text_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_text".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    assert!(!is_error(&text_result));

    let text_content = extract_text(&text_result);
    let parsed: TextResult = serde_json::from_str(&text_content)?;
    assert!(
        parsed.text.contains("UNIQUE_TEST_STRING"),
        "Expected 'UNIQUE_TEST_STRING' in output, got: {:?}",
        parsed.text
    );

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_press_key_and_send_text() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch bash
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "bash",
                "args": ["--norc", "--noprofile"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id.clone();

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Send text
    let send_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo SEND_TEXT_TEST"
            })),
        })
        .await?;

    assert!(!is_error(&send_result));

    // Press Enter key
    let press_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_press_key".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "key": "Enter"
            })),
        })
        .await?;

    assert!(!is_error(&press_result));

    // Wait for output
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Get text and verify
    let text_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_text".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    let text_content = extract_text(&text_result);
    let parsed: TextResult = serde_json::from_str(&text_content)?;
    assert!(
        parsed.text.contains("SEND_TEXT_TEST"),
        "Expected 'SEND_TEXT_TEST' in output"
    );

    // Clean up - send exit and close
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "exit\n"
            })),
        })
        .await
        .ok();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_snapshot() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch echo command
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "echo",
                "args": ["SNAPSHOT_TEST_DATA"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    // Wait for output
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Get snapshot
    let snapshot_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_snapshot".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    assert!(!is_error(&snapshot_result));

    let snapshot_text = extract_text(&snapshot_result);
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_text)?;

    // Should have yaml field
    assert!(parsed.get("yaml").is_some());
    let yaml = parsed["yaml"].as_str().unwrap();
    assert!(!yaml.is_empty());

    // Should have span_count field
    assert!(parsed.get("span_count").is_some());

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_get_session_info() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch a command
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "echo",
                "args": ["test"],
                "cols": 100,
                "rows": 30
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    // Get session info
    let info_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_get_session".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    assert!(!is_error(&info_result));

    let info_text = extract_text(&info_result);
    let info: serde_json::Value = serde_json::from_str(&info_text)?;

    assert_eq!(info["session_id"].as_str().unwrap(), session_id);
    assert_eq!(info["command"].as_str().unwrap(), "echo");
    assert_eq!(info["cols"].as_u64().unwrap(), 100);
    assert_eq!(info["rows"].as_u64().unwrap(), 30);

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_invalid_key() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch bash
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "bash",
                "args": ["--norc", "--noprofile"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Try to press an invalid key
    let press_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_press_key".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "key": "InvalidKeyName123"
            })),
        })
        .await?;

    assert!(is_error(&press_result));

    let error_text = extract_text(&press_result);
    assert!(error_text.contains("Invalid key"));

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_resize() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch bash
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "bash",
                "args": ["--norc", "--noprofile"],
                "cols": 80,
                "rows": 24
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Resize the terminal
    let resize_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_resize".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "cols": 120,
                "rows": 40
            })),
        })
        .await?;

    assert!(!is_error(&resize_result));

    // Verify the resize took effect
    let info_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_get_session".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    let info_text = extract_text(&info_result);
    let info: serde_json::Value = serde_json::from_str(&info_text)?;

    assert_eq!(info["cols"].as_u64().unwrap(), 120);
    assert_eq!(info["rows"].as_u64().unwrap(), 40);

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_close_nonexistent_session() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Try to close a non-existent session
    let close_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": "does-not-exist-12345"
            })),
        })
        .await?;

    assert!(is_error(&close_result));

    let error_text = extract_text(&close_result);
    assert!(error_text.contains("Session not found"));

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_screenshot() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch echo command
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "echo",
                "args": ["Screenshot Test"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    // Wait for output
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Get screenshot
    let screenshot_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_screenshot".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await?;

    assert!(!is_error(&screenshot_result));

    let screenshot_text = extract_text(&screenshot_result);
    let parsed: serde_json::Value = serde_json::from_str(&screenshot_text)?;

    // Should have data field with base64 content
    assert!(parsed.get("data").is_some());
    let data = parsed["data"].as_str().unwrap();
    assert!(!data.is_empty());

    // Should have format field
    assert_eq!(parsed["format"].as_str().unwrap(), "png");

    // Should have dimensions
    assert!(parsed["width"].as_u64().unwrap() > 0);
    assert!(parsed["height"].as_u64().unwrap() > 0);

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

#[tokio::test]
async fn test_wait_for_text() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;

    // Launch bash
    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "bash",
                "args": ["--norc", "--noprofile"]
            })),
        })
        .await?;

    let launch_text = extract_text(&launch_result);
    let launch: LaunchResult = serde_json::from_str(&launch_text)?;
    let session_id = launch.session_id;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send echo command
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo WAIT_FOR_TEXT_TEST\n"
            })),
        })
        .await
        .ok();

    // Wait for the text to appear
    let wait_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_wait_for_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "WAIT_FOR_TEXT_TEST",
                "timeout_ms": 5000
            })),
        })
        .await?;

    assert!(!is_error(&wait_result));

    let wait_text = extract_text(&wait_result);
    let parsed: serde_json::Value = serde_json::from_str(&wait_text)?;
    assert!(parsed["found"].as_bool().unwrap());

    // Clean up
    client
        .call_tool(CallToolRequestParam {
            name: "tui_close".into(),
            arguments: make_args(json!({
                "session_id": session_id
            })),
        })
        .await
        .ok();

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}
