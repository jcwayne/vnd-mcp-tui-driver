//! Tests for session recording functionality
//!
//! These tests verify the asciicast v3 recording feature of the MCP server.
//! Recording captures terminal sessions to .cast files for playback with asciinema.

use mcp_tui_driver::TuiServer;
use mcp_tui_driver::tools::LaunchResult;
use rmcp::model::CallToolRequestParam;
use rmcp::{ClientHandler, ServiceExt};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

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

/// Test helper to launch bash with optional recording
async fn launch_bash_with_recording(
    client: &rmcp::service::RunningService<rmcp::service::RoleClient, DummyClientHandler>,
    recording_path: Option<&str>,
    include_input: bool,
) -> anyhow::Result<String> {
    let mut args = json!({
        "command": "bash",
        "args": ["--norc", "--noprofile"],
        "cols": 80,
        "rows": 24
    });

    if let Some(path) = recording_path {
        args["recording"] = json!({
            "enabled": true,
            "outputPath": path,
            "includeInput": include_input
        });
    }

    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(args),
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

    // Give time for recording to flush
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    client.cancel().await?;
    server_handle.abort();
    Ok(())
}

/// Parse a .cast file and return its lines
fn parse_cast_file(path: &PathBuf) -> anyhow::Result<Vec<serde_json::Value>> {
    let content = fs::read_to_string(path)?;
    let mut result = Vec::new();
    for line in content.lines() {
        if !line.is_empty() {
            let parsed: serde_json::Value = serde_json::from_str(line)?;
            result.push(parsed);
        }
    }
    Ok(result)
}

// =============================================================================
// Recording Tests
// =============================================================================

/// Test 1: Recording disabled by default - no file created without recording param
#[tokio::test]
async fn test_recording_disabled_by_default() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("should_not_exist.cast");

    let (client, server_handle) = setup_client_server().await?;

    // Launch bash WITHOUT recording configuration
    let session_id = launch_bash_with_recording(&client, None, false).await?;

    // Send some text
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo test\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Verify no recording file was created
    assert!(
        !recording_path.exists(),
        "Recording file should not exist when recording is disabled"
    );

    Ok(())
}

/// Test 2: Recording creates file at outputPath when enabled
#[tokio::test]
async fn test_recording_creates_file() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("session.cast");

    let (client, server_handle) = setup_client_server().await?;

    // Launch bash WITH recording enabled
    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

    // Send some text to generate content
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo hello\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Verify recording file was created
    assert!(
        recording_path.exists(),
        "Recording file should exist at: {:?}",
        recording_path
    );

    // Verify file is not empty
    let content = fs::read_to_string(&recording_path)?;
    assert!(!content.is_empty(), "Recording file should not be empty");

    Ok(())
}

/// Test 3: Recording has valid v3 JSON header with version:3, term:{cols,rows}, timestamp, command
#[tokio::test]
async fn test_recording_valid_header() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("header_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

    cleanup(client, &session_id, server_handle).await?;

    // Parse the recording file
    let lines = parse_cast_file(&recording_path)?;
    assert!(!lines.is_empty(), "Recording should have at least a header");

    // Verify header format
    let header = &lines[0];

    // Check version is 3
    assert_eq!(
        header["version"].as_u64(),
        Some(3),
        "Header should have version: 3"
    );

    // Check term has cols and rows
    assert!(header["term"].is_object(), "Header should have term object");
    assert_eq!(
        header["term"]["cols"].as_u64(),
        Some(80),
        "Header term should have cols: 80"
    );
    assert_eq!(
        header["term"]["rows"].as_u64(),
        Some(24),
        "Header term should have rows: 24"
    );

    // Check timestamp exists and is a number
    assert!(
        header["timestamp"].is_u64(),
        "Header should have timestamp as a number"
    );

    // Check command is "bash"
    assert_eq!(
        header["command"].as_str(),
        Some("bash"),
        "Header should have command: bash"
    );

    Ok(())
}

/// Test 4: Output events ("o") are captured in the recording
#[tokio::test]
async fn test_recording_output_events() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("output_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

    // Send a command that produces output
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo OUTPUT_MARKER\n"
            })),
        })
        .await?;

    // Wait for output to be recorded
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and look for output events
    let lines = parse_cast_file(&recording_path)?;

    // Find output events (type "o")
    let output_events: Vec<&serde_json::Value> = lines
        .iter()
        .skip(1) // Skip header
        .filter(|line| line.is_array() && line[1].as_str() == Some("o"))
        .collect();

    assert!(
        !output_events.is_empty(),
        "Recording should contain output events"
    );

    // Check that at least one output event contains our marker
    let contains_marker = output_events
        .iter()
        .any(|event| {
            event[2]
                .as_str()
                .map(|s| s.contains("OUTPUT_MARKER"))
                .unwrap_or(false)
        });

    assert!(
        contains_marker,
        "Recording should contain OUTPUT_MARKER in output events"
    );

    Ok(())
}

/// Test 5: Input events ("i") are recorded when includeInput=true
#[tokio::test]
async fn test_recording_input_events_when_enabled() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("input_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    // Launch with includeInput=true
    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        true, // includeInput = true
    )
    .await?;

    // Send some input
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo INPUT_RECORDED\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and look for input events
    let lines = parse_cast_file(&recording_path)?;

    // Find input events (type "i")
    let input_events: Vec<&serde_json::Value> = lines
        .iter()
        .skip(1) // Skip header
        .filter(|line| line.is_array() && line[1].as_str() == Some("i"))
        .collect();

    assert!(
        !input_events.is_empty(),
        "Recording should contain input events when includeInput=true"
    );

    // Check that input events contain our text
    let contains_echo = input_events
        .iter()
        .any(|event| {
            event[2]
                .as_str()
                .map(|s| s.contains("echo") || s.contains("INPUT_RECORDED"))
                .unwrap_or(false)
        });

    assert!(
        contains_echo,
        "Recording should contain our input text in input events"
    );

    Ok(())
}

/// Test 6: No input events ("i") are recorded when includeInput=false (default)
#[tokio::test]
async fn test_recording_no_input_events_by_default() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("no_input_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    // Launch with includeInput=false (default)
    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false, // includeInput = false
    )
    .await?;

    // Send some input
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo NO_INPUT_TEST\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and look for input events
    let lines = parse_cast_file(&recording_path)?;

    // Find input events (type "i")
    let input_events: Vec<&serde_json::Value> = lines
        .iter()
        .skip(1) // Skip header
        .filter(|line| line.is_array() && line[1].as_str() == Some("i"))
        .collect();

    assert!(
        input_events.is_empty(),
        "Recording should NOT contain input events when includeInput=false, found: {:?}",
        input_events
    );

    Ok(())
}

/// Test 7: Resize events ("r") are recorded with format "COLSxROWS"
#[tokio::test]
async fn test_recording_resize_events() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("resize_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

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

    assert!(!is_error(&resize_result), "Resize should succeed");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and look for resize events
    let lines = parse_cast_file(&recording_path)?;

    // Find resize events (type "r")
    let resize_events: Vec<&serde_json::Value> = lines
        .iter()
        .skip(1) // Skip header
        .filter(|line| line.is_array() && line[1].as_str() == Some("r"))
        .collect();

    assert!(
        !resize_events.is_empty(),
        "Recording should contain resize events"
    );

    // Check format is "COLSxROWS"
    let has_correct_format = resize_events
        .iter()
        .any(|event| event[2].as_str() == Some("120x40"));

    assert!(
        has_correct_format,
        "Resize event should have format '120x40', found: {:?}",
        resize_events
    );

    Ok(())
}

/// Test 8: Exit event ("x") is recorded on close with exit code
#[tokio::test]
async fn test_recording_exit_event() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("exit_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

    // Close the session (this should record an exit event)
    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and look for exit events
    let lines = parse_cast_file(&recording_path)?;

    // Find exit events (type "x")
    let exit_events: Vec<&serde_json::Value> = lines
        .iter()
        .skip(1) // Skip header
        .filter(|line| line.is_array() && line[1].as_str() == Some("x"))
        .collect();

    assert!(
        !exit_events.is_empty(),
        "Recording should contain an exit event on close"
    );

    // Exit event data should be a string representing the exit code
    let exit_event = exit_events.last().unwrap();
    let exit_code_str = exit_event[2].as_str();
    assert!(
        exit_code_str.is_some(),
        "Exit event should have an exit code string"
    );

    // Verify it's a valid number string
    let _exit_code: i32 = exit_code_str
        .unwrap()
        .parse()
        .expect("Exit code should be a valid integer string");

    Ok(())
}

/// Test 9: Error on invalid path (e.g., /nonexistent/dir/file.cast)
#[tokio::test]
async fn test_recording_invalid_path() -> anyhow::Result<()> {
    let (client, server_handle) = setup_client_server().await?;

    // Try to launch with an invalid recording path
    let invalid_path = "/nonexistent/directory/that/does/not/exist/recording.cast";

    let launch_result = client
        .call_tool(CallToolRequestParam {
            name: "tui_launch".into(),
            arguments: make_args(json!({
                "command": "bash",
                "args": ["--norc", "--noprofile"],
                "cols": 80,
                "rows": 24,
                "recording": {
                    "enabled": true,
                    "outputPath": invalid_path,
                    "includeInput": false
                }
            })),
        })
        .await?;

    // Should return an error
    assert!(
        is_error(&launch_result),
        "Launch with invalid recording path should fail, got: {}",
        extract_text(&launch_result)
    );

    let error_text = extract_text(&launch_result);
    assert!(
        error_text.contains("Error") || error_text.contains("error") || error_text.contains("Failed"),
        "Error message should indicate failure, got: {}",
        error_text
    );

    client.cancel().await?;
    server_handle.abort();

    Ok(())
}

/// Test 10: Event intervals are positive floats
#[tokio::test]
async fn test_recording_intervals() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("intervals_test.cast");

    let (client, server_handle) = setup_client_server().await?;

    let session_id = launch_bash_with_recording(
        &client,
        Some(recording_path.to_str().unwrap()),
        false,
    )
    .await?;

    // Send multiple commands with some delay between them
    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo first\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    client
        .call_tool(CallToolRequestParam {
            name: "tui_send_text".into(),
            arguments: make_args(json!({
                "session_id": session_id,
                "text": "echo second\n"
            })),
        })
        .await?;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    cleanup(client, &session_id, server_handle).await?;

    // Parse recording and check intervals
    let lines = parse_cast_file(&recording_path)?;

    // Check all event lines (skip header)
    for (i, line) in lines.iter().enumerate().skip(1) {
        if line.is_array() {
            let interval = line[0].as_f64();
            assert!(
                interval.is_some(),
                "Event {} should have a numeric interval, got: {:?}",
                i,
                line[0]
            );

            let interval_value = interval.unwrap();
            assert!(
                interval_value >= 0.0,
                "Event {} interval should be non-negative, got: {}",
                i,
                interval_value
            );

            // Intervals should be reasonable (less than 10 seconds for our test)
            assert!(
                interval_value < 10.0,
                "Event {} interval should be reasonable, got: {}",
                i,
                interval_value
            );
        }
    }

    Ok(())
}
