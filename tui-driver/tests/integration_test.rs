//! Integration tests for TuiDriver

use tui_driver::{driver::LaunchOptions, TuiDriver};

#[tokio::test]
async fn test_launch_and_text_snapshot() {
    // Launch a simple command that outputs known text
    let options = LaunchOptions::new("echo").args(vec!["Hello, TUI!".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for output to be processed
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let text = driver.text();
    assert!(
        text.contains("Hello, TUI!"),
        "Expected 'Hello, TUI!' in output, got: {:?}",
        text
    );

    driver.close().await.expect("Failed to close");
}

#[tokio::test]
async fn test_launch_interactive_command() {
    // Launch bash and send a command
    let options = LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Send a command
    driver.send_text("echo TEST_OUTPUT\n").expect("Failed to send text");

    // Wait for output
    let found = driver
        .wait_for_text("TEST_OUTPUT", 2000)
        .await
        .expect("Wait failed");

    assert!(found, "Expected to find TEST_OUTPUT in screen");

    // Clean exit
    driver.send_text("exit\n").expect("Failed to send exit");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    driver.close().await.expect("Failed to close");
}

#[tokio::test]
async fn test_wait_for_idle() {
    let options = LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for initial output to settle
    driver
        .wait_for_idle(100, 5000)
        .await
        .expect("Wait for idle failed");

    // Screen should be stable now
    let text1 = driver.text();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let text2 = driver.text();

    assert_eq!(text1, text2, "Screen should be stable after wait_for_idle");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}
