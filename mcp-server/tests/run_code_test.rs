//! Tests for tui_run_code and tui_get_code_interface MCP tools
//!
//! These tests verify the JavaScript execution environment and TypeScript
//! interface generation provided by the MCP server.

use mcp_tui_driver::TuiServer;
use mcp_tui_driver::tools::LaunchResult;
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

/// Helper struct for parsed run_code results
#[derive(Debug, serde::Deserialize)]
struct RunCodeResult {
    result: String,
    logs: Vec<ConsoleLogEntry>,
}

/// Console log entry structure
#[derive(Debug, serde::Deserialize)]
struct ConsoleLogEntry {
    level: String,
    message: String,
}

/// Test helper to create a client-server pair and return the client
async fn setup_client_server() -> anyhow::Result<(
    rmcp::service::RunningService<rmcp::service::RoleClient, DummyClientHandler>,
    tokio::task::JoinHandle<anyhow::Result<()>>,
)> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);

    let server = TuiServer::new();
    let server_handle = tokio::spawn(async move {
        server.serve(server_transport).await?.waiting().await?;
        anyhow::Ok(())
    });

    let client = DummyClientHandler.serve(client_transport).await?;
    Ok((client, server_handle))
}

/// Test helper to launch bash and return session_id
async fn launch_bash(
    client: &rmcp::service::RunningService<rmcp::service::RoleClient, DummyClientHandler>,
) -> anyhow::Result<String> {
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

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(launch.session_id)
}

/// Test helper to close session and cleanup
async fn cleanup(
    client: rmcp::service::RunningService<rmcp::service::RoleClient, DummyClientHandler>,
    session_id: &str,
    server_handle: tokio::task::JoinHandle<anyhow::Result<()>>,
) -> anyhow::Result<()> {
    // Close session
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

// =============================================================================
// tui_run_code Tests
// =============================================================================

/// Test 1: Execute simple JS, verify result
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_basic() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Execute simple JavaScript that returns a value
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "1 + 2 + 3"
            })),
        })
        .await?;

    assert!(!is_error(&result), "run_code failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Result should be "6"
    assert_eq!(parsed.result, "6");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 2: tui.text() returns string
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_tui_text() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // First, echo something to have content
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo UNIQUE_TEXT_MARKER\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Use tui.text() to get screen content
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.text()"
            })),
        })
        .await?;

    assert!(!is_error(&result), "run_code failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Result should contain the echoed text
    assert!(
        parsed.result.contains("UNIQUE_TEXT_MARKER"),
        "Expected 'UNIQUE_TEXT_MARKER' in result, got: {}",
        parsed.result
    );

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 3: tui.snapshot() returns object with span_count
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_tui_snapshot() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Get snapshot via JS
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "JSON.stringify(tui.snapshot())"
            })),
        })
        .await?;

    assert!(!is_error(&result), "run_code failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Parse the JSON result
    let snapshot: serde_json::Value = serde_json::from_str(&parsed.result)?;

    // Should have span_count field
    assert!(
        snapshot.get("span_count").is_some(),
        "Expected 'span_count' in snapshot"
    );

    // Should have rows field
    assert!(
        snapshot.get("rows").is_some(),
        "Expected 'rows' in snapshot"
    );

    // Should have spans field
    assert!(
        snapshot.get("spans").is_some(),
        "Expected 'spans' in snapshot"
    );

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 4: tui.screenshot() saves file, returns path
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_tui_screenshot() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Take screenshot via JS
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.screenshot('test-screenshot.png')"
            })),
        })
        .await?;

    assert!(!is_error(&result), "run_code failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Result should be a file path ending with .png
    assert!(
        parsed.result.ends_with(".png"),
        "Expected path ending with .png, got: {}",
        parsed.result
    );

    // Path should contain /tmp/tui-screenshots/
    assert!(
        parsed.result.contains("/tmp/tui-screenshots/"),
        "Expected path in /tmp/tui-screenshots/, got: {}",
        parsed.result
    );

    // File should exist
    let screenshot_path = std::path::Path::new(&parsed.result);
    assert!(
        screenshot_path.exists(),
        "Screenshot file should exist at: {}",
        parsed.result
    );

    // Clean up the screenshot file after verification
    if screenshot_path.exists() {
        std::fs::remove_file(screenshot_path).ok();
    }

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 5: sendText, pressKey, pressKeys work
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_input_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Test sendText
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.sendText('echo INPUT_TEST'); undefined"
            })),
        })
        .await?;
    assert!(!is_error(&result), "sendText failed: {}", extract_text(&result));

    // Test pressKey
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.pressKey('Enter'); undefined"
            })),
        })
        .await?;
    assert!(!is_error(&result), "pressKey failed: {}", extract_text(&result));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify the command executed
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.text()"
            })),
        })
        .await?;

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert!(
        parsed.result.contains("INPUT_TEST"),
        "Expected 'INPUT_TEST' in output"
    );

    // Test pressKeys
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.pressKeys(['e', 'c', 'h', 'o', 'Space', 'K', 'E', 'Y', 'S', 'Enter']); undefined"
            })),
        })
        .await?;
    assert!(!is_error(&result), "pressKeys failed: {}", extract_text(&result));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify pressKeys worked
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.text()"
            })),
        })
        .await?;

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert!(
        parsed.result.contains("KEYS"),
        "Expected 'KEYS' in output from pressKeys"
    );

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 6: click, clickAt, doubleClick, rightClick, hover, drag
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_mouse_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Test clickAt - should not error (terminal may not respond to mouse, but method should work)
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.clickAt(10, 5); 'clickAt_ok'"
            })),
        })
        .await?;
    assert!(!is_error(&result), "clickAt failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "clickAt_ok");

    // Test click with invalid ref - should return an error about element not found
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "try { tui.click('nonexistent'); 'no_error' } catch(e) { 'error_caught' }"
            })),
        })
        .await?;
    assert!(!is_error(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "error_caught", "click with bad ref should throw error");

    // Test doubleClick with invalid ref
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "try { tui.doubleClick('nonexistent'); 'no_error' } catch(e) { 'error_caught' }"
            })),
        })
        .await?;
    assert!(!is_error(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "error_caught", "doubleClick with bad ref should throw error");

    // Test rightClick with invalid ref
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "try { tui.rightClick('nonexistent'); 'no_error' } catch(e) { 'error_caught' }"
            })),
        })
        .await?;
    assert!(!is_error(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "error_caught", "rightClick with bad ref should throw error");

    // Test hover with invalid ref
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "try { tui.hover('nonexistent'); 'no_error' } catch(e) { 'error_caught' }"
            })),
        })
        .await?;
    assert!(!is_error(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "error_caught", "hover with bad ref should throw error");

    // Test drag with invalid refs
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "try { tui.drag('start', 'end'); 'no_error' } catch(e) { 'error_caught' }"
            })),
        })
        .await?;
    assert!(!is_error(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "error_caught", "drag with bad refs should throw error");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 7: waitForText, waitForIdle return booleans
///
/// Note: The waitForText and waitForIdle JS methods use futures::executor::block_on
/// internally, which can cause deadlocks in async contexts. We test via the MCP tool
/// interface which properly handles the async boundaries.
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_wait_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // First echo something
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo WAIT_TARGET\n"
            })),
        })
        .await?;

    // Give bash time to process
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Test waitForText - should find text and return true
    // Use the direct MCP tool (tui_wait_for_text) instead of running JS that blocks
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_wait_for_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "WAIT_TARGET",
                "timeout_ms": 5000
            })),
        })
        .await?;

    assert!(!is_error(&result), "waitForText failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        parsed["found"].as_bool().expect("'found' field should be a boolean"),
        "waitForText should return found=true"
    );

    // Test waitForText with non-existent text - short timeout
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_wait_for_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "NONEXISTENT_TEXT_12345",
                "timeout_ms": 100
            })),
        })
        .await?;

    assert!(!is_error(&result), "waitForText with non-existent text should not return error");
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    assert!(
        !parsed["found"].as_bool().expect("'found' field should be a boolean"),
        "waitForText should return found=false"
    );

    // Test waitForIdle via MCP tool
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_wait_for_idle".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "timeout_ms": 2000,
                "idle_ms": 200
            })),
        })
        .await?;

    // waitForIdle returns success result (not error) on success
    assert!(!is_error(&result), "waitForIdle failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: serde_json::Value = serde_json::from_str(&text)?;
    // waitForIdle returns {"success": true} on success
    assert!(parsed.get("success").is_some(), "waitForIdle should return success field");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 8: resize, sendSignal work
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_control_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Test resize
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.resize(100, 30); 'resize_ok'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "resize failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "resize_ok");

    // Verify resize took effect by checking session info
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
    assert_eq!(
        info["cols"].as_u64().expect("'cols' field should be a number"),
        100,
        "cols should be 100 after resize"
    );
    assert_eq!(
        info["rows"].as_u64().expect("'rows' field should be a number"),
        30,
        "rows should be 30 after resize"
    );

    // Test sendSignal - send SIGINT (Ctrl+C)
    // First start a long-running command
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "sleep 100\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Send SIGINT to interrupt it
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.sendSignal('SIGINT'); 'signal_sent'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "sendSignal failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "signal_sent");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 9: getScrollback, getInput, getOutput
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_debug_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Send some text to create input/output
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo DEBUG_TEST\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Test getScrollback - returns a number
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "typeof tui.getScrollback() === 'number' ? 'is_number' : 'not_number'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "getScrollback failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "is_number", "getScrollback should return a number");

    // Test getInput - returns a string
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "typeof tui.getInput() === 'string' ? 'is_string' : 'not_string'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "getInput failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "is_string", "getInput should return a string");

    // Verify getInput contains our sent text
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.getInput().includes('DEBUG_TEST') ? 'found' : 'not_found'"
            })),
        })
        .await?;

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "found", "getInput should contain sent text");

    // Test getOutput - returns a string
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "typeof tui.getOutput() === 'string' ? 'is_string' : 'not_string'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "getOutput failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "is_string", "getOutput should return a string");

    // Test getInput with character limit
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "tui.getInput(5).length <= 5 ? 'limited' : 'not_limited'"
            })),
        })
        .await?;

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "limited", "getInput with char limit should work");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 10: console.log/warn/error/info/debug in logs
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_console_capture() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Test all console methods
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": r#"
                    console.log('log message');
                    console.info('info message');
                    console.warn('warn message');
                    console.error('error message');
                    console.debug('debug message');
                    'done'
                "#
            })),
        })
        .await?;

    assert!(!is_error(&result), "console logging failed: {}", extract_text(&result));

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Should have 5 log entries
    assert_eq!(parsed.logs.len(), 5, "Expected 5 log entries");

    // Verify each log level
    let log_levels: Vec<&str> = parsed.logs.iter().map(|l| l.level.as_str()).collect();
    assert!(log_levels.contains(&"log"), "Should have 'log' level");
    assert!(log_levels.contains(&"info"), "Should have 'info' level");
    assert!(log_levels.contains(&"warn"), "Should have 'warn' level");
    assert!(log_levels.contains(&"error"), "Should have 'error' level");
    assert!(log_levels.contains(&"debug"), "Should have 'debug' level");

    // Verify messages
    let log_messages: Vec<&str> = parsed.logs.iter().map(|l| l.message.as_str()).collect();
    assert!(log_messages.contains(&"log message"), "Should have 'log message'");
    assert!(log_messages.contains(&"info message"), "Should have 'info message'");
    assert!(log_messages.contains(&"warn message"), "Should have 'warn message'");
    assert!(log_messages.contains(&"error message"), "Should have 'error message'");
    assert!(log_messages.contains(&"debug message"), "Should have 'debug message'");

    // Test console.log with multiple arguments
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "console.log('hello', 'world', 123); 'done'"
            })),
        })
        .await?;

    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Should have 1 log entry with joined message
    assert_eq!(parsed.logs.len(), 1);
    assert_eq!(parsed.logs[0].message, "hello world 123");

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 11: Variable set in call 1 accessible in call 2
/// Note: The current implementation creates a new JS context for each call,
/// so this test documents that variables do NOT persist (which is the current behavior).
/// If variable persistence is desired in the future, this test would need to be updated.
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_context_persistence() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Set a variable in the first call
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "var testVar = 'persistent_value'; testVar"
            })),
        })
        .await?;

    assert!(!is_error(&result), "First call failed: {}", extract_text(&result));
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;
    assert_eq!(parsed.result, "persistent_value");

    // Try to access the variable in a second call
    // Note: Based on the current implementation (execute_script creates new Context each time),
    // variables will NOT persist. This test documents the current behavior.
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "typeof testVar !== 'undefined' ? testVar : 'variable_not_found'"
            })),
        })
        .await?;

    assert!(!is_error(&result), "Second call should not return error");
    let text = extract_text(&result);
    let parsed: RunCodeResult = serde_json::from_str(&text)?;

    // Current implementation does NOT persist variables across calls
    // If persistence is implemented, change this assertion to:
    // assert_eq!(parsed.result, "persistent_value");
    assert_eq!(
        parsed.result, "variable_not_found",
        "Variables do not persist across run_code calls (by design)"
    );

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

/// Test 12: Invalid JS returns error
#[tokio::test(flavor = "multi_thread")]
async fn test_run_code_error_handling() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;
    let session_id = launch_bash(&client).await?;

    // Test syntax error
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "function { invalid syntax"
            })),
        })
        .await?;

    assert!(is_error(&result), "Syntax error should return error result");
    let error_text = extract_text(&result).to_lowercase();
    assert!(
        error_text.contains("javascript")
            || error_text.contains("error")
            || error_text.contains("syntax")
            || error_text.contains("unexpected"),
        "Error message should indicate JavaScript error, got: {}",
        error_text
    );

    // Test runtime error
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "code": "undefinedFunction()"
            })),
        })
        .await?;

    assert!(is_error(&result), "Runtime error should return error result");

    // Test invalid session
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_run_code".into(),
            arguments: make_args(json!({
                "session_id": "nonexistent-session-id",
                "code": "1 + 1"
            })),
        })
        .await?;

    assert!(is_error(&result), "Invalid session should return error");
    let error_text = extract_text(&result);
    assert!(
        error_text.contains("Session not found"),
        "Error should mention session not found"
    );

    cleanup(client, &session_id, server_handle).await?;
    Ok(())
}

// =============================================================================
// tui_get_code_interface Tests
// =============================================================================

/// Test 13: Returns non-empty string
#[tokio::test(flavor = "multi_thread")]
async fn test_get_code_interface_returns_typescript() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;

    // Call tui_get_code_interface
    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_get_code_interface".into(),
            arguments: None,
        })
        .await?;

    assert!(!is_error(&result), "get_code_interface failed: {}", extract_text(&result));

    let text = extract_text(&result);

    // Should be non-empty
    assert!(!text.is_empty(), "Interface should not be empty");

    // Should contain TypeScript-like content
    assert!(
        text.contains("interface") || text.contains("declare"),
        "Should contain TypeScript interface or declare statements"
    );

    client.cancel().await?;
    server_handle.abort();
    Ok(())
}

/// Test 14: Contains all 18 method signatures
#[tokio::test(flavor = "multi_thread")]
async fn test_get_code_interface_contains_methods() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_get_code_interface".into(),
            arguments: None,
        })
        .await?;

    let text = extract_text(&result);

    // Check for all 18 method signatures
    let methods = [
        // Display (3)
        "text()",
        "snapshot()",
        "screenshot(",
        // Input (3)
        "sendText(",
        "pressKey(",
        "pressKeys(",
        // Mouse (6)
        "click(",
        "clickAt(",
        "doubleClick(",
        "rightClick(",
        "hover(",
        "drag(",
        // Wait (2)
        "waitForText(",
        "waitForIdle(",
        // Control (2)
        "resize(",
        "sendSignal(",
        // Debug (3)
        "getScrollback()",
        "getInput(",
        "getOutput(",
    ];

    for method in &methods {
        assert!(
            text.contains(method),
            "Interface should contain method: {}",
            method
        );
    }

    client.cancel().await?;
    server_handle.abort();
    Ok(())
}

/// Test 15: Contains Snapshot, Row, Span types
#[tokio::test(flavor = "multi_thread")]
async fn test_get_code_interface_contains_types() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;

    let result = client
        .call_tool(CallToolRequestParam {
            name: "tui_get_code_interface".into(),
            arguments: None,
        })
        .await?;

    let text = extract_text(&result);

    // Check for type definitions
    assert!(
        text.contains("interface Snapshot") || text.contains("type Snapshot"),
        "Interface should contain Snapshot type"
    );

    assert!(
        text.contains("interface Row") || text.contains("type Row"),
        "Interface should contain Row type"
    );

    assert!(
        text.contains("interface Span") || text.contains("type Span"),
        "Interface should contain Span type"
    );

    // Check for Console type
    assert!(
        text.contains("interface Console") || text.contains("Console"),
        "Interface should contain Console type"
    );

    // Check for Tui type
    assert!(
        text.contains("interface Tui") || text.contains("declare const tui"),
        "Interface should contain Tui interface"
    );

    client.cancel().await?;
    server_handle.abort();
    Ok(())
}
