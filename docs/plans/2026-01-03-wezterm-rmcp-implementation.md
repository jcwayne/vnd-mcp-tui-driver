# wezterm-term + rmcp Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace vt100 with wezterm-term for terminal emulation, and hand-rolled JSON-RPC with rmcp SDK for MCP protocol.

**Architecture:** Two-phase migration maintaining backward compatibility. Phase 1 swaps terminal emulator while preserving public API. Phase 2 replaces MCP protocol layer while keeping tool logic intact.

**Tech Stack:** wezterm-term (git tag 20240203-110809-5046fc22), rmcp SDK, clap CLI parser

---

## Phase 1: wezterm-term Migration

### Task 1.1: Add wezterm-term dependency

**Files:**
- Modify: `tui-driver/Cargo.toml`

**Step 1: Update Cargo.toml with wezterm-term git dependency**

Edit `tui-driver/Cargo.toml`:

```toml
[dependencies]
portable-pty = "0.8"
termwiz = { git = "https://github.com/wezterm/wezterm", tag = "20240203-110809-5046fc22" }
wezterm-term = { git = "https://github.com/wezterm/wezterm", tag = "20240203-110809-5046fc22" }
tokio = { version = "1", features = ["full", "sync", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
uuid = { version = "1", features = ["v4"] }
parking_lot = "0.12"
image = "0.25"
base64 = "0.22"
```

Note: Keep vt100 for now until migration is complete.

**Step 2: Verify compilation**

Run: `cd tui-driver && cargo build`
Expected: Compiles successfully (may take time to fetch git deps)

**Step 3: Commit**

```bash
git add tui-driver/Cargo.toml Cargo.lock
git commit -m "build: add wezterm-term and termwiz dependencies"
```

---

### Task 1.2: Create wezterm terminal wrapper module

**Files:**
- Create: `tui-driver/src/terminal.rs`
- Modify: `tui-driver/src/lib.rs`

**Step 1: Write the failing test for terminal wrapper**

Create `tui-driver/src/terminal.rs`:

```rust
//! Terminal emulator wrapper using wezterm-term

use parking_lot::Mutex;
use std::sync::Arc;
use termwiz::surface::CursorShape;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

/// Configuration for wezterm terminal
#[derive(Clone)]
pub struct TuiTerminalConfig {
    scrollback_lines: usize,
}

impl TuiTerminalConfig {
    pub fn new(scrollback_lines: usize) -> Self {
        Self { scrollback_lines }
    }
}

impl TerminalConfiguration for TuiTerminalConfig {
    fn scrollback_size(&self) -> usize {
        self.scrollback_lines
    }

    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}

/// Wrapper around wezterm Terminal for TUI driver
pub struct TuiTerminal {
    terminal: Arc<Mutex<Terminal>>,
}

impl TuiTerminal {
    /// Create a new terminal with given dimensions
    pub fn new(rows: u16, cols: u16, scrollback_lines: usize) -> Self {
        let size = TerminalSize {
            rows: rows as usize,
            cols: cols as usize,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 96,
        };

        let config = Arc::new(TuiTerminalConfig::new(scrollback_lines));
        let terminal = Terminal::new(size, config, "TuiDriver", "1.0", Box::new(Vec::new()));

        Self {
            terminal: Arc::new(Mutex::new(terminal)),
        }
    }

    /// Process bytes from PTY output
    pub fn advance_bytes(&self, bytes: &[u8]) {
        let mut term = self.terminal.lock();
        term.advance_bytes(bytes);
    }

    /// Resize the terminal
    pub fn resize(&self, rows: u16, cols: u16) {
        let size = TerminalSize {
            rows: rows as usize,
            cols: cols as usize,
            pixel_width: 0,
            pixel_height: 0,
            dpi: 96,
        };
        let mut term = self.terminal.lock();
        term.resize(size);
    }

    /// Get the screen for reading
    pub fn with_screen<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&wezterm_term::Screen) -> R,
    {
        let term = self.terminal.lock();
        f(term.screen())
    }

    /// Get terminal dimensions
    pub fn size(&self) -> (u16, u16) {
        let term = self.terminal.lock();
        let size = term.get_size();
        (size.rows as u16, size.cols as u16)
    }

    /// Get scrollback line count
    pub fn scrollback(&self) -> usize {
        let term = self.terminal.lock();
        term.screen().scrollback_rows()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_creation() {
        let term = TuiTerminal::new(24, 80, 500);
        assert_eq!(term.size(), (24, 80));
    }

    #[test]
    fn test_terminal_advance_bytes() {
        let term = TuiTerminal::new(24, 80, 500);
        term.advance_bytes(b"Hello World");

        term.with_screen(|screen| {
            // Verify text was written
            let cell = screen.get_cell(0, 0);
            assert!(cell.is_some());
        });
    }

    #[test]
    fn test_terminal_resize() {
        let term = TuiTerminal::new(24, 80, 500);
        term.resize(40, 120);
        assert_eq!(term.size(), (40, 120));
    }
}
```

**Step 2: Add module to lib.rs**

Edit `tui-driver/src/lib.rs`, add after other module declarations:

```rust
mod terminal;
pub use terminal::TuiTerminal;
```

**Step 3: Run test to verify it compiles and passes**

Run: `cd tui-driver && cargo test terminal`
Expected: All 3 terminal tests pass

**Step 4: Commit**

```bash
git add tui-driver/src/terminal.rs tui-driver/src/lib.rs
git commit -m "feat: add wezterm-term terminal wrapper"
```

---

### Task 1.3: Update snapshot.rs for wezterm-term cell API

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Write failing test for wezterm cell extraction**

Add to `tui-driver/src/snapshot.rs` tests section:

```rust
#[test]
fn test_build_snapshot_wezterm() {
    use crate::terminal::TuiTerminal;

    let term = TuiTerminal::new(24, 80, 0);
    term.advance_bytes(b"Hello World");

    term.with_screen(|screen| {
        let snapshot = build_snapshot_from_wezterm(screen);
        assert!(!snapshot.is_empty());
        assert_eq!(snapshot.span_count(), 1);
        assert_eq!(snapshot.spans[0].text, "Hello World");
    });
}
```

**Step 2: Run test to verify it fails**

Run: `cd tui-driver && cargo test test_build_snapshot_wezterm`
Expected: FAIL with "cannot find function `build_snapshot_from_wezterm`"

**Step 3: Add wezterm-term imports and color conversion**

Add at top of `tui-driver/src/snapshot.rs`:

```rust
use wezterm_term::{
    CellAttributes, Screen as WeztermScreen,
    color::ColorSpec,
};
```

Add after existing `color_to_string` function:

```rust
/// Convert wezterm ColorSpec to string representation
fn wezterm_color_to_string(color: &ColorSpec) -> Option<String> {
    match color {
        ColorSpec::Default => None,
        ColorSpec::PaletteIndex(i) => Some(format!("color{}", i)),
        ColorSpec::TrueColor(c) => {
            let (r, g, b, _) = c.to_tuple_rgba8();
            Some(format!("#{:02x}{:02x}{:02x}", r, g, b))
        }
    }
}
```

**Step 4: Add wezterm style comparison function**

Add after `same_style` function:

```rust
/// Check if two wezterm cells have the same styling
fn same_wezterm_style(a: &CellAttributes, b: &CellAttributes) -> bool {
    a.intensity() == b.intensity()
        && a.italic() == b.italic()
        && a.underline() == b.underline()
        && a.reverse() == b.reverse()
        && a.foreground() == b.foreground()
        && a.background() == b.background()
        && a.strikethrough() == b.strikethrough()
        && a.blink() == b.blink()
        && a.hyperlink() == b.hyperlink()
}
```

**Step 5: Add wezterm empty cell check**

Add after `is_empty_cell` function:

```rust
/// Check if a wezterm cell is effectively empty
fn is_wezterm_cell_empty(text: &str, attrs: &CellAttributes) -> bool {
    let is_blank = text.is_empty() || text == " ";
    let is_default = attrs.intensity() == wezterm_term::Intensity::Normal
        && !attrs.italic()
        && attrs.underline() == wezterm_term::Underline::None
        && !attrs.reverse()
        && !attrs.strikethrough()
        && attrs.blink() == wezterm_term::Blink::None
        && *attrs.foreground() == ColorSpec::Default
        && *attrs.background() == ColorSpec::Default
        && attrs.hyperlink().is_none();

    is_blank && is_default
}
```

**Step 6: Add build_snapshot_from_wezterm function**

Add after `build_snapshot` function:

```rust
/// Build a Snapshot from a wezterm Screen
pub fn build_snapshot_from_wezterm(screen: &WeztermScreen) -> Snapshot {
    let num_rows = screen.physical_rows;
    let num_cols = screen.physical_cols;
    let mut rows: Vec<Row> = Vec::new();
    let mut span_counter: usize = 0;

    for row_idx in 0..num_rows {
        let mut row_spans: Vec<Span> = Vec::new();
        let mut col_idx: usize = 0;

        let line = screen.get_line(row_idx as i32);
        let line = match line {
            Some(l) => l,
            None => continue,
        };

        while col_idx < num_cols {
            let cell = line.get_cell(col_idx);
            let text = cell.str();
            let attrs = cell.attrs();

            // Skip empty cells
            if is_wezterm_cell_empty(text, attrs) {
                col_idx += 1;
                continue;
            }

            // Start a new span
            let start_col = col_idx;
            let mut span_text = String::new();
            let first_attrs = attrs.clone();

            // Collect consecutive cells with same style
            while col_idx < num_cols {
                let current_cell = line.get_cell(col_idx);
                let current_text = current_cell.str();
                let current_attrs = current_cell.attrs();

                if !same_wezterm_style(&first_attrs, current_attrs) {
                    if is_wezterm_cell_empty(current_text, current_attrs) {
                        // Look ahead for continuation
                        let mut found = false;
                        for ahead in (col_idx + 1)..num_cols {
                            let ahead_cell = line.get_cell(ahead);
                            let ahead_text = ahead_cell.str();
                            let ahead_attrs = ahead_cell.attrs();
                            if is_wezterm_cell_empty(ahead_text, ahead_attrs) {
                                continue;
                            }
                            if same_wezterm_style(&first_attrs, ahead_attrs) {
                                found = true;
                            }
                            break;
                        }
                        if found {
                            span_text.push_str(if current_text.is_empty() { " " } else { current_text });
                            col_idx += 1;
                            continue;
                        }
                    }
                    break;
                }

                span_text.push_str(if current_text.is_empty() { " " } else { current_text });
                col_idx += 1;
            }

            // Trim and create span
            let trimmed = span_text.trim_end().to_string();
            if trimmed.is_empty() {
                continue;
            }

            span_counter += 1;
            let ref_id = format!("s{}", span_counter);
            let width = trimmed.chars().count() as u16;

            let mut span = Span::new(
                ref_id,
                trimmed,
                start_col as u16 + 1,
                row_idx as u16 + 1,
                width,
            );

            // Add styling
            if first_attrs.intensity() == wezterm_term::Intensity::Bold {
                span.bold = Some(true);
            }
            if first_attrs.italic() {
                span.italic = Some(true);
            }
            if first_attrs.underline() != wezterm_term::Underline::None {
                span.underline = Some(true);
            }
            if first_attrs.reverse() {
                span.inverse = Some(true);
            }
            if let Some(fg) = wezterm_color_to_string(first_attrs.foreground()) {
                span.fg = Some(fg);
            }
            if let Some(bg) = wezterm_color_to_string(first_attrs.background()) {
                span.bg = Some(bg);
            }

            row_spans.push(span);
        }

        if !row_spans.is_empty() {
            rows.push(Row::with_spans(row_idx as u16 + 1, row_spans));
        }
    }

    let yaml = generate_yaml(&rows);
    let spans: Vec<Span> = rows.iter().flat_map(|r| r.spans.clone()).collect();

    Snapshot {
        rows,
        spans,
        yaml: Some(yaml),
    }
}
```

**Step 7: Run test to verify it passes**

Run: `cd tui-driver && cargo test test_build_snapshot_wezterm`
Expected: PASS

**Step 8: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat: add wezterm-term snapshot extraction"
```

---

### Task 1.4: Add extended span attributes

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Write failing test for extended attributes**

Add to tests:

```rust
#[test]
fn test_extended_span_attributes() {
    let span = Span::new("s1", "test", 1, 1, 4)
        .with_strikethrough(true)
        .with_blink("slow".to_string())
        .with_underline_style("curly".to_string())
        .with_link("https://example.com".to_string());

    assert_eq!(span.strikethrough, Some(true));
    assert_eq!(span.blink, Some("slow".to_string()));
    assert_eq!(span.underline_style, Some("curly".to_string()));
    assert_eq!(span.link, Some("https://example.com".to_string()));
}
```

**Step 2: Run test to verify it fails**

Run: `cd tui-driver && cargo test test_extended_span`
Expected: FAIL with "no method named `with_strikethrough`"

**Step 3: Add new fields to Span struct**

Update the Span struct definition:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub ref_id: String,
    pub text: String,
    pub x: u16,
    pub y: u16,
    pub width: u16,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,

    /// Extended underline style: single, double, curly
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_style: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<bool>,

    /// Strikethrough text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strikethrough: Option<bool>,

    /// Blink style: slow, rapid
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blink: Option<String>,

    /// Hyperlink URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,

    /// Image reference ID (for image placeholders)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    /// Image size in cells (WxH format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_size: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
}
```

**Step 4: Update Span::new to initialize new fields**

```rust
impl Span {
    pub fn new(
        ref_id: impl Into<String>,
        text: impl Into<String>,
        x: u16,
        y: u16,
        width: u16,
    ) -> Self {
        Self {
            ref_id: ref_id.into(),
            text: text.into(),
            x,
            y,
            width,
            bold: None,
            italic: None,
            underline: None,
            underline_style: None,
            inverse: None,
            strikethrough: None,
            blink: None,
            link: None,
            image: None,
            image_size: None,
            fg: None,
            bg: None,
        }
    }
```

**Step 5: Add builder methods for new attributes**

Add after existing builder methods:

```rust
    pub fn with_strikethrough(mut self, strikethrough: bool) -> Self {
        self.strikethrough = Some(strikethrough);
        self
    }

    pub fn with_blink(mut self, blink: impl Into<String>) -> Self {
        self.blink = Some(blink.into());
        self
    }

    pub fn with_underline_style(mut self, style: impl Into<String>) -> Self {
        self.underline_style = Some(style.into());
        self
    }

    pub fn with_link(mut self, url: impl Into<String>) -> Self {
        self.link = Some(url.into());
        self
    }

    pub fn with_image(mut self, image_ref: impl Into<String>, size: impl Into<String>) -> Self {
        self.image = Some(image_ref.into());
        self.image_size = Some(size.into());
        self
    }
```

**Step 6: Run test to verify it passes**

Run: `cd tui-driver && cargo test test_extended_span`
Expected: PASS

**Step 7: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat: add extended span attributes for styling"
```

---

### Task 1.5: Update build_snapshot_from_wezterm with extended attributes

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Write test for strikethrough extraction**

Add to tests:

```rust
#[test]
fn test_wezterm_strikethrough() {
    use crate::terminal::TuiTerminal;

    let term = TuiTerminal::new(24, 80, 0);
    // ESC[9m = strikethrough on
    term.advance_bytes(b"\x1b[9mStrike\x1b[0m");

    term.with_screen(|screen| {
        let snapshot = build_snapshot_from_wezterm(screen);
        assert_eq!(snapshot.spans[0].strikethrough, Some(true));
    });
}
```

**Step 2: Update build_snapshot_from_wezterm to extract extended attributes**

In the styling section of build_snapshot_from_wezterm, after setting inverse, add:

```rust
            // Extended attributes
            if first_attrs.strikethrough() {
                span.strikethrough = Some(true);
            }
            match first_attrs.blink() {
                wezterm_term::Blink::Slow => span.blink = Some("slow".to_string()),
                wezterm_term::Blink::Rapid => span.blink = Some("rapid".to_string()),
                wezterm_term::Blink::None => {}
            }
            match first_attrs.underline() {
                wezterm_term::Underline::Single => span.underline_style = Some("single".to_string()),
                wezterm_term::Underline::Double => span.underline_style = Some("double".to_string()),
                wezterm_term::Underline::Curly => span.underline_style = Some("curly".to_string()),
                wezterm_term::Underline::Dotted => span.underline_style = Some("dotted".to_string()),
                wezterm_term::Underline::Dashed => span.underline_style = Some("dashed".to_string()),
                wezterm_term::Underline::None => {}
            }
            if let Some(hyperlink) = first_attrs.hyperlink() {
                span.link = Some(hyperlink.uri().to_string());
            }
```

**Step 3: Run tests**

Run: `cd tui-driver && cargo test wezterm`
Expected: All wezterm tests pass

**Step 4: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat: extract extended styling from wezterm cells"
```

---

### Task 1.6: Update generate_yaml for extended attributes

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Write test for YAML with extended attributes**

```rust
#[test]
fn test_yaml_extended_attributes() {
    let rows = vec![Row::with_spans(
        1,
        vec![Span::new("s1", "link", 1, 1, 4)
            .with_link("https://example.com".to_string())
            .with_strikethrough(true)
            .with_blink("slow".to_string())],
    )];

    let yaml = generate_yaml(&rows);
    assert!(yaml.contains("strikethrough"));
    assert!(yaml.contains("blink=slow"));
    assert!(yaml.contains("link=https://example.com"));
}
```

**Step 2: Update generate_yaml function**

Replace the attrs building section in generate_yaml:

```rust
            if span.bold == Some(true) {
                attrs.push("bold".to_string());
            }
            if span.italic == Some(true) {
                attrs.push("italic".to_string());
            }
            if span.underline == Some(true) {
                attrs.push("underline".to_string());
            }
            if let Some(ref style) = span.underline_style {
                attrs.push(format!("underline={}", style));
            }
            if span.inverse == Some(true) {
                attrs.push("inverse".to_string());
            }
            if span.strikethrough == Some(true) {
                attrs.push("strikethrough".to_string());
            }
            if let Some(ref blink) = span.blink {
                attrs.push(format!("blink={}", blink));
            }
            if let Some(ref fg) = span.fg {
                attrs.push(format!("fg={}", fg));
            }
            if let Some(ref bg) = span.bg {
                attrs.push(format!("bg={}", bg));
            }
            if let Some(ref link) = span.link {
                attrs.push(format!("link={}", link));
            }
            if let Some(ref img) = span.image {
                attrs.push(format!("image={}", img));
                if let Some(ref size) = span.image_size {
                    attrs.push(format!("size={}", size));
                }
            }
```

**Step 3: Run test**

Run: `cd tui-driver && cargo test test_yaml_extended`
Expected: PASS

**Step 4: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat: extend YAML output with new attributes"
```

---

### Task 1.7: Switch driver.rs to use TuiTerminal

**Files:**
- Modify: `tui-driver/src/driver.rs`

**Step 1: Update imports**

Replace vt100 import with TuiTerminal:

```rust
use crate::terminal::TuiTerminal;
use crate::snapshot::{build_snapshot_from_wezterm, render_screenshot_from_wezterm, Screenshot, Snapshot};
```

**Step 2: Update TuiDriver struct**

Replace the parser field:

```rust
    /// Terminal emulator state
    terminal: Arc<TuiTerminal>,
```

Remove the SCROLLBACK_LINES constant (moved to terminal.rs).

**Step 3: Update TuiDriver::launch**

Replace parser initialization:

```rust
        // Initialize terminal emulator
        let terminal = Arc::new(TuiTerminal::new(
            options.rows,
            options.cols,
            500, // scrollback lines
        ));
```

Update the reader thread to use terminal:

```rust
        let reader_thread = {
            let terminal = Arc::clone(&terminal);
            let last_update = Arc::clone(&last_update);
            let running = Arc::clone(&running);
            let output_buffer = Arc::clone(&output_buffer);

            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let mut master_reader = master_reader;

                while running.load(Ordering::SeqCst) {
                    match master_reader.read(&mut buf) {
                        Ok(0) => {
                            running.store(false, Ordering::SeqCst);
                            break;
                        }
                        Ok(n) => {
                            let text = String::from_utf8_lossy(&buf[..n]);
                            output_buffer.push_str(&text);
                            terminal.advance_bytes(&buf[..n]);
                            last_update.store(current_timestamp_ms(), Ordering::SeqCst);
                        }
                        Err(e) => {
                            if e.kind() != std::io::ErrorKind::WouldBlock {
                                running.store(false, Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                }
            })
        };
```

Update struct initialization:

```rust
            terminal,
```

**Step 4: Update text() method**

```rust
    pub fn text(&self) -> String {
        self.terminal.with_screen(|screen| {
            let mut result = String::new();
            let num_rows = screen.physical_rows;
            let num_cols = screen.physical_cols;

            for row_idx in 0..num_rows {
                if let Some(line) = screen.get_line(row_idx as i32) {
                    let row_text: String = (0..num_cols)
                        .map(|col| {
                            let text = line.get_cell(col).str();
                            if text.is_empty() { " " } else { text }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    result.push_str(row_text.trim_end());
                }
                result.push('\n');
            }

            while result.ends_with("\n\n") {
                result.pop();
            }

            result
        })
    }
```

**Step 5: Update snapshot() method**

```rust
    pub fn snapshot(&self) -> Snapshot {
        self.terminal.with_screen(|screen| {
            build_snapshot_from_wezterm(screen)
        })
    }
```

**Step 6: Update screenshot() method**

```rust
    pub fn screenshot(&self) -> Screenshot {
        self.terminal.with_screen(|screen| {
            render_screenshot_from_wezterm(screen)
        })
    }
```

**Step 7: Update resize() method**

```rust
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        if !self.is_running() {
            return Err(TuiError::SessionClosed);
        }

        let master = self.master.lock();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TuiError::ResizeFailed(e.to_string()))?;

        self.cols.store(cols, Ordering::SeqCst);
        self.rows.store(rows, Ordering::SeqCst);
        self.terminal.resize(rows, cols);

        Ok(())
    }
```

**Step 8: Update get_scrollback() method**

```rust
    pub fn get_scrollback(&self) -> usize {
        self.terminal.scrollback()
    }
```

**Step 9: Run all tests**

Run: `cd tui-driver && cargo test`
Expected: All tests pass

**Step 10: Commit**

```bash
git add tui-driver/src/driver.rs
git commit -m "refactor: switch TuiDriver to use TuiTerminal wrapper"
```

---

### Task 1.8: Add render_screenshot_from_wezterm

**Files:**
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Add wezterm screenshot function**

Add after render_screenshot:

```rust
/// Render wezterm screen to PNG image
pub fn render_screenshot_from_wezterm(screen: &WeztermScreen) -> Screenshot {
    let rows = screen.physical_rows;
    let cols = screen.physical_cols;
    let char_width = 10u32;
    let char_height = 20u32;
    let width = cols as u32 * char_width;
    let height = rows as u32 * char_height;

    let mut img: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));

    for row in 0..rows {
        if let Some(line) = screen.get_line(row as i32) {
            for col in 0..cols {
                let cell = line.get_cell(col);
                let contents = cell.str();
                if !contents.is_empty() && contents != " " {
                    let px = col as u32 * char_width;
                    let py = row as u32 * char_height;

                    let fg = match cell.attrs().foreground() {
                        ColorSpec::Default => Rgba([255, 255, 255, 255]),
                        ColorSpec::PaletteIndex(i) => ansi_to_rgba(*i),
                        ColorSpec::TrueColor(c) => {
                            let (r, g, b, a) = c.to_tuple_rgba8();
                            Rgba([r, g, b, a])
                        }
                    };

                    for dx in 2..char_width - 2 {
                        for dy in 4..char_height - 4 {
                            if px + dx < width && py + dy < height {
                                img.put_pixel(px + dx, py + dy, fg);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut buffer = Vec::new();
    {
        use image::codecs::png::PngEncoder;
        use image::ImageEncoder;
        let encoder = PngEncoder::new(&mut buffer);
        encoder
            .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
            .expect("Failed to encode PNG");
    }

    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD.encode(&buffer);

    Screenshot {
        data,
        format: "png".to_string(),
        width,
        height,
    }
}
```

**Step 2: Run tests**

Run: `cd tui-driver && cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add tui-driver/src/snapshot.rs
git commit -m "feat: add wezterm screenshot rendering"
```

---

### Task 1.9: Remove vt100 dependency

**Files:**
- Modify: `tui-driver/Cargo.toml`
- Modify: `tui-driver/src/snapshot.rs`

**Step 1: Remove vt100 from Cargo.toml**

Edit `tui-driver/Cargo.toml`, remove:

```toml
vt100 = "0.15"
```

**Step 2: Remove vt100 code from snapshot.rs**

Remove:
- `use vt100::Screen;`
- `fn color_to_string(color: vt100::Color)`
- `fn same_style(a: &vt100::Cell, b: &vt100::Cell)`
- `fn is_empty_cell(cell: &vt100::Cell)`
- `pub fn build_snapshot(screen: &Screen)`
- `pub fn render_screenshot(screen: &Screen)`
- All tests that use `vt100::Parser` directly

Keep the wezterm versions of these functions.

**Step 3: Update exports**

Rename wezterm functions to main names:
- `build_snapshot_from_wezterm` -> `build_snapshot`
- `render_screenshot_from_wezterm` -> `render_screenshot`

**Step 4: Update driver.rs imports**

Change snapshot imports to use renamed functions:

```rust
use crate::snapshot::{build_snapshot, render_screenshot, Screenshot, Snapshot};
```

Update snapshot() and screenshot() methods to call renamed functions.

**Step 5: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 6: Run integration tests**

Run: `cargo test --test integration_test`
Expected: All 8 integration tests pass

**Step 7: Commit**

```bash
git add -A
git commit -m "refactor: remove vt100, use wezterm-term exclusively"
```

---

## Phase 2: rmcp Migration

### Task 2.1: Add rmcp and clap dependencies

**Files:**
- Modify: `mcp-server/Cargo.toml`

**Step 1: Update dependencies**

Add to `mcp-server/Cargo.toml`:

```toml
[dependencies]
tui-driver = { path = "../tui-driver" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full", "io-std"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1"
async-trait = "0.1"
boa_engine = "0.20"
rmcp = { version = "0.1", features = ["server"] }
clap = { version = "4", features = ["derive"] }
```

**Step 2: Verify build**

Run: `cargo build -p mcp-tui-driver`
Expected: Compiles successfully

**Step 3: Commit**

```bash
git add mcp-server/Cargo.toml Cargo.lock
git commit -m "build: add rmcp and clap dependencies"
```

---

### Task 2.2: Create TuiServer struct with rmcp

**Files:**
- Create: `mcp-server/src/server.rs`
- Modify: `mcp-server/src/main.rs`

**Step 1: Create server.rs with TuiServer struct**

Create `mcp-server/src/server.rs`:

```rust
//! TUI MCP Server implementation using rmcp

use anyhow::Result;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolResult, Tool};
use rmcp::schemars::JsonSchema;
use rmcp::{tool, ServerHandler};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tui_driver::TuiDriver;

/// TUI MCP Server
#[derive(Clone)]
pub struct TuiServer {
    sessions: Arc<Mutex<HashMap<String, TuiDriver>>>,
}

impl TuiServer {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for TuiServer {
    fn default() -> Self {
        Self::new()
    }
}

#[rmcp::async_trait]
impl ServerHandler for TuiServer {
    fn get_info(&self) -> rmcp::model::ServerInfo {
        rmcp::model::ServerInfo {
            name: "mcp-tui-driver".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }
}
```

**Step 2: Add module to main.rs**

Add at top of main.rs:

```rust
mod server;
use server::TuiServer;
```

**Step 3: Run build**

Run: `cargo build -p mcp-tui-driver`
Expected: Compiles

**Step 4: Commit**

```bash
git add mcp-server/src/server.rs mcp-server/src/main.rs
git commit -m "feat: add TuiServer struct for rmcp"
```

---

### Task 2.3: Implement tui_launch tool with rmcp

**Files:**
- Modify: `mcp-server/src/server.rs`

**Step 1: Add launch tool parameter struct**

Add after TuiServer impl:

```rust
/// Parameters for tui_launch
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LaunchParams {
    /// Command to execute
    pub command: String,

    /// Command arguments
    #[serde(default)]
    pub args: Vec<String>,

    /// Terminal width in columns
    #[serde(default = "default_cols")]
    pub cols: u16,

    /// Terminal height in rows
    #[serde(default = "default_rows")]
    pub rows: u16,
}

fn default_cols() -> u16 { 80 }
fn default_rows() -> u16 { 24 }
```

**Step 2: Implement tui_launch with #[tool] macro**

Add tool implementation block:

```rust
#[tool(tool_box)]
impl TuiServer {
    /// Launch a new TUI application session
    #[tool(description = "Launch a new TUI application session")]
    pub async fn tui_launch(
        &self,
        #[tool(aggr)] params: LaunchParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let options = tui_driver::LaunchOptions::new(&params.command)
            .args(params.args)
            .size(params.cols, params.rows);

        match TuiDriver::launch(options).await {
            Ok(driver) => {
                let session_id = driver.session_id().to_string();
                let info = driver.info();

                let mut sessions = self.sessions.lock().await;
                sessions.insert(session_id.clone(), driver);

                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    serde_json::to_string_pretty(&info).unwrap(),
                )]))
            }
            Err(e) => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                format!("Failed to launch: {}", e),
            )])),
        }
    }
}
```

**Step 3: Run build**

Run: `cargo build -p mcp-tui-driver`
Expected: Compiles

**Step 4: Commit**

```bash
git add mcp-server/src/server.rs
git commit -m "feat: implement tui_launch tool with rmcp"
```

---

### Task 2.4: Implement remaining session tools

**Files:**
- Modify: `mcp-server/src/server.rs`

**Step 1: Add session parameter structs**

```rust
/// Common session identifier parameter
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionParams {
    /// Session identifier returned by tui_launch
    pub session_id: String,
}
```

**Step 2: Implement tui_close**

Add to tool impl block:

```rust
    /// Close a TUI session
    #[tool(description = "Close a TUI session")]
    pub async fn tui_close(
        &self,
        #[tool(aggr)] params: SessionParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let mut sessions = self.sessions.lock().await;

        match sessions.remove(&params.session_id) {
            Some(driver) => {
                // Save debug data before closing
                let _ = save_closed_session(&driver);
                let _ = driver.close().await;
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    "Session closed",
                )]))
            }
            None => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                "Session not found",
            )])),
        }
    }
```

**Step 3: Implement tui_list_sessions**

```rust
    /// List all active TUI sessions
    #[tool(description = "List all active TUI sessions")]
    pub async fn tui_list_sessions(&self) -> Result<CallToolResult, rmcp::Error> {
        let sessions = self.sessions.lock().await;
        let ids: Vec<_> = sessions.keys().cloned().collect();

        Ok(CallToolResult::success(vec![rmcp::model::Content::text(
            serde_json::to_string_pretty(&ids).unwrap(),
        )]))
    }
```

**Step 4: Implement tui_get_session**

```rust
    /// Get information about a TUI session
    #[tool(description = "Get information about a TUI session")]
    pub async fn tui_get_session(
        &self,
        #[tool(aggr)] params: SessionParams,
    ) -> Result<CallToolResult, rmcp::Error> {
        let sessions = self.sessions.lock().await;

        match sessions.get(&params.session_id) {
            Some(driver) => {
                let info = driver.info();
                Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                    serde_json::to_string_pretty(&info).unwrap(),
                )]))
            }
            None => Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                "Session not found",
            )])),
        }
    }
```

**Step 5: Run build**

Run: `cargo build -p mcp-tui-driver`
Expected: Compiles

**Step 6: Commit**

```bash
git add mcp-server/src/server.rs
git commit -m "feat: implement session management tools"
```

---

### Task 2.5-2.10: Implement remaining tools

Continue implementing all 22 tools following the same pattern:

- Display: `tui_text`, `tui_snapshot`, `tui_screenshot`
- Input: `tui_press_key`, `tui_press_keys`, `tui_send_text`
- Mouse: `tui_click`, `tui_click_at`, `tui_double_click`, `tui_right_click`
- Wait: `tui_wait_for_text`, `tui_wait_for_idle`
- Control: `tui_resize`, `tui_send_signal`
- Script: `tui_run_code`
- Debug: `tui_get_input`, `tui_get_output`, `tui_get_scrollback`

Each tool follows the pattern:
1. Define params struct with JsonSchema
2. Add #[tool] method
3. Access session from HashMap
4. Return CallToolResult

---

### Task 2.11: Add CLI with clap

**Files:**
- Modify: `mcp-server/src/main.rs`

**Step 1: Add CLI struct**

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "mcp-tui-driver")]
#[command(about = "MCP server for TUI automation")]
struct Cli {
    /// Run in SSE mode instead of stdio
    #[arg(long)]
    sse: bool,

    /// Port for SSE server
    #[arg(long, default_value = "8080")]
    port: u16,
}
```

**Step 2: Update main function**

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let server = TuiServer::new();

    if cli.sse {
        // SSE transport not implemented yet
        anyhow::bail!("SSE transport not yet implemented");
    } else {
        rmcp::serve_stdio(server).await?;
    }

    Ok(())
}
```

**Step 3: Run build**

Run: `cargo build -p mcp-tui-driver`
Expected: Compiles

**Step 4: Commit**

```bash
git add mcp-server/src/main.rs
git commit -m "feat: add clap CLI with transport selection"
```

---

### Task 2.12: Remove old JSON-RPC code

**Files:**
- Modify: `mcp-server/src/main.rs`
- Delete: `mcp-server/src/tools.rs` (if separate)

**Step 1: Remove old code**

Remove all hand-rolled JSON-RPC types and handlers:
- `JsonRpcRequest`
- `JsonRpcResponse`
- `JsonRpcError`
- `SessionManager`
- `McpServer`
- Tool dispatch code

Keep:
- `ClosedSessionData` and persistence functions
- Boa engine integration for `tui_run_code`

**Step 2: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: remove hand-rolled JSON-RPC, use rmcp exclusively"
```

---

### Task 2.13: Add rmcp test suite

**Files:**
- Create: `mcp-server/tests/rmcp_test.rs`

**Step 1: Create test file**

```rust
//! rmcp integration tests for TUI MCP server

use mcp_tui_driver::TuiServer;
use rmcp::testing::TestClient;
use serde_json::json;

#[tokio::test]
async fn test_launch_and_close() {
    let server = TuiServer::new();
    let client = TestClient::new(server);

    // Launch echo command
    let result = client
        .call_tool(
            "tui_launch",
            json!({
                "command": "echo",
                "args": ["hello"]
            }),
        )
        .await
        .expect("launch should succeed");

    assert!(result.is_success());

    // Extract session_id from response
    let content = result.content[0].as_text().unwrap();
    let info: serde_json::Value = serde_json::from_str(content).unwrap();
    let session_id = info["session_id"].as_str().unwrap();

    // Close session
    let close_result = client
        .call_tool("tui_close", json!({ "session_id": session_id }))
        .await
        .expect("close should succeed");

    assert!(close_result.is_success());
}

#[tokio::test]
async fn test_invalid_session() {
    let server = TuiServer::new();
    let client = TestClient::new(server);

    let result = client
        .call_tool("tui_text", json!({ "session_id": "nonexistent" }))
        .await
        .expect("should return error result");

    assert!(!result.is_success());
}

#[tokio::test]
async fn test_list_sessions() {
    let server = TuiServer::new();
    let client = TestClient::new(server);

    // Initially empty
    let result = client
        .call_tool("tui_list_sessions", json!({}))
        .await
        .expect("list should succeed");

    let content = result.content[0].as_text().unwrap();
    let sessions: Vec<String> = serde_json::from_str(content).unwrap();
    assert!(sessions.is_empty());
}
```

**Step 2: Run tests**

Run: `cargo test -p mcp-tui-driver`
Expected: All rmcp tests pass

**Step 3: Commit**

```bash
git add mcp-server/tests/rmcp_test.rs
git commit -m "test: add rmcp integration test suite"
```

---

## Final Verification

### Task: Run full test suite and manual test

**Step 1: Run all tests**

Run: `cargo test`
Expected: All 57+ tests pass

**Step 2: Build release**

Run: `cargo build --release -p mcp-tui-driver`
Expected: Compiles successfully

**Step 3: Manual test with Claude Code**

Test configuration in Claude Code settings:
```json
{
  "mcpServers": {
    "tui-driver": {
      "command": "path/to/mcp-tui-driver"
    }
  }
}
```

Test basic workflow:
1. Launch vim
2. Type text
3. Save and quit
4. Verify file created

**Step 4: Final commit**

```bash
git add -A
git commit -m "chore: complete wezterm-term + rmcp migration"
```

---

## Summary

| Phase | Tasks | Est. Lines Changed |
|-------|-------|-------------------|
| Phase 1 | 1.1-1.9 | +400, -300 |
| Phase 2 | 2.1-2.13 | +400, -1300 |
| **Total** | 22 tasks | **Net: -800 lines** |

The migration reduces code complexity significantly by:
- Replacing custom vt100 wrapper with maintained wezterm-term
- Replacing 1900 lines of hand-rolled JSON-RPC with ~200 lines of rmcp macros
- Adding extended terminal features (hyperlinks, images, styling)
