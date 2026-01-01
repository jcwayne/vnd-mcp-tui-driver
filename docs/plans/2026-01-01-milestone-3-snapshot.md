# Milestone 3: The Snapshot - Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add accessibility-style snapshots with span extraction, YAML format, and PNG screenshot support.

**Architecture:** Span extractor analyzes vt100 screen cells, groups them by styling, assigns refs, outputs YAML. Screenshot renders terminal to PNG using image crate.

**Tech Stack:** Rust, vt100, image crate for PNG rendering.

---

## Context

The snapshot should group consecutive characters with same styling into "spans". Each span gets a reference like "s1", "s2" for LLM interaction. Format is YAML similar to Playwright accessibility snapshots.

Example output:
```yaml
- row 1:
  - span "htop - ubuntu" [bold] [inverse] [ref=s1] (1,1)
- row 2:
  - span "CPU[" [ref=s2] (1,2)
  - span "||||||||" [fg=green] [ref=s3] (5,2)
```

---

### Task 1: Add Snapshot Types and Span Structure

**Files:**
- Create: `tui-driver/src/snapshot.rs`
- Modify: `tui-driver/src/lib.rs`
- Modify: `tui-driver/Cargo.toml` (add serde derive feature)

**Step 1: Create snapshot.rs with types**

```rust
//! Accessibility-style snapshot generation

use serde::{Deserialize, Serialize};

/// A span of text with consistent styling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Reference ID for this span (e.g., "s1", "s2")
    pub ref_id: String,
    /// The text content
    pub text: String,
    /// 1-based column position
    pub x: u16,
    /// 1-based row position
    pub y: u16,
    /// Width in characters
    pub width: u16,
    /// Styling attributes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
}

/// A row containing spans
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    /// 1-based row number
    pub row: u16,
    /// Spans in this row
    pub spans: Vec<Span>,
}

/// Complete accessibility snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Rows with content
    pub rows: Vec<Row>,
    /// All spans for quick lookup
    pub spans: Vec<Span>,
    /// YAML representation
    pub yaml: String,
}

impl Snapshot {
    /// Get span by reference ID
    pub fn get_by_ref(&self, ref_id: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.ref_id == ref_id)
    }

    /// Get span by text content (first match)
    pub fn get_by_text(&self, text: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.text.contains(text))
    }
}
```

**Step 2: Add serde derive feature to Cargo.toml**

In tui-driver/Cargo.toml dependencies:
```toml
serde = { version = "1.0", features = ["derive"] }
```

**Step 3: Export from lib.rs**

```rust
pub mod snapshot;
pub use snapshot::{Snapshot, Span, Row};
```

**Step 4: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 5: Commit**

```bash
git add tui-driver/src/snapshot.rs tui-driver/src/lib.rs tui-driver/Cargo.toml
git commit -m "feat(tui-driver): add snapshot types (Span, Row, Snapshot)"
```

---

### Task 2: Implement Span Extraction from vt100 Screen

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Add span extraction logic**

Add helper functions and SnapshotBuilder:

```rust
use vt100::Screen;

/// Color to string conversion
fn color_to_string(color: vt100::Color) -> Option<String> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(format!("color{}", i)),
        vt100::Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
    }
}

/// Check if two cells have same styling
fn same_style(a: &vt100::Cell, b: &vt100::Cell) -> bool {
    a.bold() == b.bold()
        && a.italic() == b.italic()
        && a.underline() == b.underline()
        && a.inverse() == b.inverse()
        && a.fgcolor() == b.fgcolor()
        && a.bgcolor() == b.bgcolor()
}

/// Build snapshot from vt100 screen
pub fn build_snapshot(screen: &Screen) -> Snapshot {
    let mut rows = Vec::new();
    let mut all_spans = Vec::new();
    let mut ref_counter = 0u32;

    let (num_rows, num_cols) = screen.size();

    for row_idx in 0..num_rows {
        let mut row_spans = Vec::new();
        let mut current_span_start: Option<u16> = None;
        let mut current_text = String::new();
        let mut current_cell: Option<vt100::Cell> = None;

        for col_idx in 0..num_cols {
            let cell = screen.cell(row_idx, col_idx);

            if let Some(cell) = cell {
                let contents = cell.contents();

                // Skip empty cells
                if contents.is_empty() || contents == " " {
                    // Flush current span if any
                    if let Some(start_col) = current_span_start {
                        if !current_text.trim().is_empty() {
                            ref_counter += 1;
                            let span = create_span(
                                ref_counter,
                                &current_text,
                                start_col,
                                row_idx as u16,
                                current_cell.as_ref(),
                            );
                            row_spans.push(span);
                        }
                    }
                    current_span_start = None;
                    current_text.clear();
                    current_cell = None;
                    continue;
                }

                match (&current_cell, current_span_start) {
                    (Some(prev_cell), Some(_)) if same_style(prev_cell, &cell) => {
                        // Continue current span
                        current_text.push_str(&contents);
                    }
                    (Some(_), Some(start_col)) => {
                        // Style changed, flush current span
                        if !current_text.trim().is_empty() {
                            ref_counter += 1;
                            let span = create_span(
                                ref_counter,
                                &current_text,
                                start_col,
                                row_idx as u16,
                                current_cell.as_ref(),
                            );
                            row_spans.push(span);
                        }
                        // Start new span
                        current_span_start = Some(col_idx);
                        current_text = contents;
                        current_cell = Some(cell.clone());
                    }
                    _ => {
                        // Start new span
                        current_span_start = Some(col_idx);
                        current_text = contents;
                        current_cell = Some(cell.clone());
                    }
                }
            }
        }

        // Flush final span in row
        if let Some(start_col) = current_span_start {
            if !current_text.trim().is_empty() {
                ref_counter += 1;
                let span = create_span(
                    ref_counter,
                    &current_text,
                    start_col,
                    row_idx as u16,
                    current_cell.as_ref(),
                );
                row_spans.push(span);
            }
        }

        if !row_spans.is_empty() {
            all_spans.extend(row_spans.clone());
            rows.push(Row {
                row: row_idx as u16 + 1, // 1-based
                spans: row_spans,
            });
        }
    }

    let yaml = generate_yaml(&rows);

    Snapshot {
        rows,
        spans: all_spans,
        yaml,
    }
}

fn create_span(
    ref_num: u32,
    text: &str,
    start_col: u16,
    row: u16,
    cell: Option<&vt100::Cell>,
) -> Span {
    let trimmed = text.trim_end();
    Span {
        ref_id: format!("s{}", ref_num),
        text: trimmed.to_string(),
        x: start_col + 1, // 1-based
        y: row + 1,       // 1-based
        width: trimmed.chars().count() as u16,
        bold: cell.filter(|c| c.bold()).map(|_| true),
        italic: cell.filter(|c| c.italic()).map(|_| true),
        underline: cell.filter(|c| c.underline()).map(|_| true),
        inverse: cell.filter(|c| c.inverse()).map(|_| true),
        fg: cell.and_then(|c| color_to_string(c.fgcolor())),
        bg: cell.and_then(|c| color_to_string(c.bgcolor())),
    }
}

fn generate_yaml(rows: &[Row]) -> String {
    let mut yaml = String::new();

    for row in rows {
        yaml.push_str(&format!("- row {}:\n", row.row));
        for span in &row.spans {
            let mut attrs = Vec::new();

            if span.bold == Some(true) {
                attrs.push("[bold]".to_string());
            }
            if span.italic == Some(true) {
                attrs.push("[italic]".to_string());
            }
            if span.underline == Some(true) {
                attrs.push("[underline]".to_string());
            }
            if span.inverse == Some(true) {
                attrs.push("[inverse]".to_string());
            }
            if let Some(ref fg) = span.fg {
                attrs.push(format!("[fg={}]", fg));
            }
            if let Some(ref bg) = span.bg {
                attrs.push(format!("[bg={}]", bg));
            }
            attrs.push(format!("[ref={}]", span.ref_id));

            let attrs_str = attrs.join(" ");
            yaml.push_str(&format!(
                "  - span \"{}\" {} ({},{})\n",
                span.text, attrs_str, span.x, span.y
            ));
        }
    }

    yaml
}
```

**Step 2: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 3: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat(tui-driver): implement span extraction from vt100 screen"
```

---

### Task 3: Add snapshot() Method to TuiDriver

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Add snapshot method**

```rust
use crate::snapshot::{build_snapshot, Snapshot};

impl TuiDriver {
    /// Get accessibility-style snapshot of current screen
    pub fn snapshot(&self) -> Snapshot {
        let parser = self.parser.lock();
        let screen = parser.screen();
        build_snapshot(screen)
    }
}
```

**Step 2: Build and test**

Run: `source ~/.cargo/env && cargo build`

**Step 3: Commit**

```bash
git add tui-driver/src/driver.rs
git commit -m "feat(tui-driver): add snapshot() method to TuiDriver"
```

---

### Task 4: Add Screenshot Support with image Crate

**Files:**
- Modify: `tui-driver/Cargo.toml`
- Modify: `tui-driver/src/snapshot.rs`
- Modify: `tui-driver/src/driver.rs`

**Step 1: Add image crate dependency**

In tui-driver/Cargo.toml:
```toml
image = "0.25"
rusttype = "0.9"
```

**Step 2: Add screenshot types and rendering**

In snapshot.rs:
```rust
use image::{ImageBuffer, Rgba, RgbaImage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Base64-encoded image data
    pub data: String,
    /// Image format
    pub format: String,
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
}

/// Render terminal screen to PNG image
pub fn render_screenshot(screen: &vt100::Screen) -> Screenshot {
    let (rows, cols) = screen.size();
    let char_width = 10u32;
    let char_height = 20u32;
    let width = cols as u32 * char_width;
    let height = rows as u32 * char_height;

    let mut img: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));

    // Simple rendering: just create a basic image with text positions marked
    // Full font rendering would require more complex setup
    for row in 0..rows {
        for col in 0..cols {
            if let Some(cell) = screen.cell(row, col) {
                let contents = cell.contents();
                if !contents.is_empty() && contents != " " {
                    // Mark cell with white if has content
                    let px = col as u32 * char_width;
                    let py = row as u32 * char_height;

                    // Get foreground color
                    let fg = match cell.fgcolor() {
                        vt100::Color::Default => Rgba([255, 255, 255, 255]),
                        vt100::Color::Idx(i) => ansi_to_rgba(i),
                        vt100::Color::Rgb(r, g, b) => Rgba([r, g, b, 255]),
                    };

                    // Fill cell area with simplified representation
                    for dx in 2..char_width-2 {
                        for dy in 4..char_height-4 {
                            if px + dx < width && py + dy < height {
                                img.put_pixel(px + dx, py + dy, fg);
                            }
                        }
                    }
                }
            }
        }
    }

    // Encode to PNG
    let mut buffer = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut buffer);
    encoder.encode(
        img.as_raw(),
        width,
        height,
        image::ExtendedColorType::Rgba8,
    ).expect("Failed to encode PNG");

    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(&buffer);

    Screenshot {
        data,
        format: "png".to_string(),
        width,
        height,
    }
}

fn ansi_to_rgba(idx: u8) -> Rgba<u8> {
    // Basic 16-color ANSI palette
    match idx {
        0 => Rgba([0, 0, 0, 255]),       // Black
        1 => Rgba([205, 0, 0, 255]),     // Red
        2 => Rgba([0, 205, 0, 255]),     // Green
        3 => Rgba([205, 205, 0, 255]),   // Yellow
        4 => Rgba([0, 0, 238, 255]),     // Blue
        5 => Rgba([205, 0, 205, 255]),   // Magenta
        6 => Rgba([0, 205, 205, 255]),   // Cyan
        7 => Rgba([229, 229, 229, 255]), // White
        8 => Rgba([127, 127, 127, 255]), // Bright Black
        9 => Rgba([255, 0, 0, 255]),     // Bright Red
        10 => Rgba([0, 255, 0, 255]),    // Bright Green
        11 => Rgba([255, 255, 0, 255]),  // Bright Yellow
        12 => Rgba([92, 92, 255, 255]),  // Bright Blue
        13 => Rgba([255, 0, 255, 255]),  // Bright Magenta
        14 => Rgba([0, 255, 255, 255]),  // Bright Cyan
        15 => Rgba([255, 255, 255, 255]),// Bright White
        // Extended 256-color palette (simplified)
        _ => Rgba([200, 200, 200, 255]),
    }
}
```

**Step 3: Add base64 dependency**

In tui-driver/Cargo.toml:
```toml
base64 = "0.22"
```

**Step 4: Add screenshot method to TuiDriver**

```rust
use crate::snapshot::{render_screenshot, Screenshot};

impl TuiDriver {
    /// Take a screenshot of current terminal state
    pub fn screenshot(&self) -> Screenshot {
        let parser = self.parser.lock();
        let screen = parser.screen();
        render_screenshot(screen)
    }
}
```

**Step 5: Build**

Run: `source ~/.cargo/env && cargo build`

**Step 6: Commit**

```bash
git add tui-driver/Cargo.toml tui-driver/src/snapshot.rs tui-driver/src/driver.rs
git commit -m "feat(tui-driver): add screenshot support with PNG rendering"
```

---

### Task 5: Add MCP Snapshot Tools

**Files:**
- Modify: `mcp-server/src/tools.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Add result types in tools.rs**

```rust
use tui_driver::{Snapshot, Screenshot};

#[derive(Debug, Serialize)]
pub struct SnapshotResult {
    pub yaml: String,
    pub spans: Vec<SpanInfo>,
}

#[derive(Debug, Serialize)]
pub struct SpanInfo {
    pub ref_id: String,
    pub text: String,
    pub x: u16,
    pub y: u16,
}

#[derive(Debug, Serialize)]
pub struct ScreenshotResult {
    pub data: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
}
```

**Step 2: Add tools to tools/list**

```rust
{
    "name": "tui_snapshot",
    "description": "Get accessibility-style snapshot with element references",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            }
        },
        "required": ["session_id"]
    }
},
{
    "name": "tui_screenshot",
    "description": "Take a PNG screenshot of the terminal",
    "inputSchema": {
        "type": "object",
        "properties": {
            "session_id": {
                "type": "string",
                "description": "Session identifier"
            }
        },
        "required": ["session_id"]
    }
}
```

**Step 3: Add handlers**

```rust
"tui_snapshot" => self.tool_snapshot(id, arguments).await,
"tui_screenshot" => self.tool_screenshot(id, arguments).await,
```

Implement handlers that call driver.snapshot() and driver.screenshot().

**Step 4: Build and test**

Run: `source ~/.cargo/env && cargo test && cargo clippy -- -D warnings`

**Step 5: Commit**

```bash
git add mcp-server/src/tools.rs mcp-server/src/main.rs
git commit -m "feat(mcp-server): add tui_snapshot and tui_screenshot tools"
```

---

### Task 6: Add Snapshot Integration Tests

**Files:**
- Modify: `tui-driver/tests/integration_test.rs`

**Step 1: Add tests**

```rust
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
    assert!(!snapshot.yaml.is_empty(), "Expected YAML output");

    // Should find our text
    let found = snapshot.get_by_text("SNAPSHOT_TEST");
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
```

**Step 2: Run tests**

Run: `source ~/.cargo/env && cargo test`

**Step 3: Commit**

```bash
git add tui-driver/tests/integration_test.rs
git commit -m "test(tui-driver): add snapshot and screenshot integration tests"
```

---

### Task 7: Final Verification

**Step 1: Run all checks**

```bash
source ~/.cargo/env
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

**Step 2: Verify MCP tools**

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | cargo run --bin mcp-tui-driver 2>/dev/null | jq '.result.tools[].name'
```

Should list 10 tools including tui_snapshot and tui_screenshot.
