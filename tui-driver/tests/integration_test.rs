//! Integration tests for TuiDriver

use tui_driver::{driver::LaunchOptions, Key, TuiDriver};

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
    let options =
        LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Send a command
    driver
        .send_text("echo TEST_OUTPUT\n")
        .expect("Failed to send text");

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
    let options =
        LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

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

#[tokio::test]
async fn test_press_key() {
    let options =
        LaunchOptions::new("bash").args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");

    // Wait for bash to start
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Type some text using press_keys
    let keys: Vec<Key> = "echo KEYTEST".chars().map(Key::Char).collect();
    driver.press_keys(&keys).expect("Failed to press keys");

    // Press Enter
    driver
        .press_key(&Key::Enter)
        .expect("Failed to press Enter");

    // Wait for output
    let found = driver
        .wait_for_text("KEYTEST", 2000)
        .await
        .expect("Wait failed");

    assert!(found, "Expected to find KEYTEST in screen");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}

#[tokio::test]
async fn test_key_parse() {
    // Test key parsing
    assert_eq!(Key::parse("Enter").unwrap(), Key::Enter);
    assert_eq!(Key::parse("escape").unwrap(), Key::Escape);
    assert_eq!(Key::parse("ArrowUp").unwrap(), Key::Up);
    assert_eq!(Key::parse("Ctrl+c").unwrap(), Key::Ctrl('c'));
    assert_eq!(Key::parse("Alt+x").unwrap(), Key::Alt('x'));
    assert_eq!(Key::parse("F1").unwrap(), Key::F1);
    assert_eq!(Key::parse("a").unwrap(), Key::Char('a'));

    // Invalid key
    assert!(Key::parse("invalid_key_name").is_err());
}

#[tokio::test]
async fn test_snapshot() {
    let options = LaunchOptions::new("bash")
        .args(vec!["--norc".to_string(), "--noprofile".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Type something to appear in snapshot
    driver.send_text("echo SNAPSHOT_TEST\n").expect("send failed");
    driver.wait_for_text("SNAPSHOT_TEST", 2000).await.expect("wait failed");

    let snapshot = driver.snapshot();

    // Should have spans
    assert!(!snapshot.spans.is_empty(), "Expected spans in snapshot");

    // Should have YAML
    assert!(snapshot.yaml.is_some(), "Expected YAML output");
    let yaml = snapshot.yaml.as_ref().unwrap();
    assert!(!yaml.is_empty(), "Expected non-empty YAML");

    // Should find our text
    let found = snapshot.get_first_by_text("SNAPSHOT_TEST");
    assert!(found.is_some(), "Expected to find SNAPSHOT_TEST span");

    driver.send_text("exit\n").ok();
    driver.close().await.ok();
}

#[tokio::test]
async fn test_screenshot() {
    let options = LaunchOptions::new("echo").args(vec!["Screenshot test".to_string()]);

    let driver = TuiDriver::launch(options).await.expect("Failed to launch");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let screenshot = driver.screenshot();

    assert_eq!(screenshot.format, "png");
    assert!(screenshot.width > 0);
    assert!(screenshot.height > 0);
    assert!(!screenshot.data.is_empty(), "Expected base64 data");

    driver.close().await.ok();
}
