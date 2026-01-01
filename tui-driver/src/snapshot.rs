//! Snapshot types for capturing terminal state.
//!
//! This module provides types for representing a snapshot of the terminal
//! screen, including individual spans with styling information, rows of
//! spans, and the complete snapshot structure.

use image::{ImageBuffer, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use vt100::Screen;

/// A span represents a contiguous piece of text with uniform styling.
///
/// Spans are the atomic units of a terminal snapshot. Each span has a
/// reference ID that can be used for element targeting in automation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Span {
    /// Unique reference ID for this span (e.g., "ref1", "ref2").
    pub ref_id: String,

    /// The text content of this span.
    pub text: String,

    /// X coordinate (column) where this span starts (0-indexed).
    pub x: u16,

    /// Y coordinate (row) where this span is located (0-indexed).
    pub y: u16,

    /// Width of this span in terminal cells.
    pub width: u16,

    /// Whether the text is bold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,

    /// Whether the text is italic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,

    /// Whether the text is underlined.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,

    /// Whether the text has inverse/reverse video.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverse: Option<bool>,

    /// Foreground color (e.g., "red", "#ff0000", or ANSI color number).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fg: Option<String>,

    /// Background color (e.g., "blue", "#0000ff", or ANSI color number).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
}

impl Span {
    /// Creates a new span with required fields only.
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
            inverse: None,
            fg: None,
            bg: None,
        }
    }

    /// Sets the bold attribute.
    pub fn with_bold(mut self, bold: bool) -> Self {
        self.bold = Some(bold);
        self
    }

    /// Sets the italic attribute.
    pub fn with_italic(mut self, italic: bool) -> Self {
        self.italic = Some(italic);
        self
    }

    /// Sets the underline attribute.
    pub fn with_underline(mut self, underline: bool) -> Self {
        self.underline = Some(underline);
        self
    }

    /// Sets the inverse attribute.
    pub fn with_inverse(mut self, inverse: bool) -> Self {
        self.inverse = Some(inverse);
        self
    }

    /// Sets the foreground color.
    pub fn with_fg(mut self, fg: impl Into<String>) -> Self {
        self.fg = Some(fg.into());
        self
    }

    /// Sets the background color.
    pub fn with_bg(mut self, bg: impl Into<String>) -> Self {
        self.bg = Some(bg.into());
        self
    }
}

/// A row represents a single line in the terminal, containing spans.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
    /// The row number (0-indexed from top of screen).
    pub row: u16,

    /// The spans contained in this row.
    pub spans: Vec<Span>,
}

impl Row {
    /// Creates a new row with the given row number.
    pub fn new(row: u16) -> Self {
        Self {
            row,
            spans: Vec::new(),
        }
    }

    /// Creates a new row with pre-populated spans.
    pub fn with_spans(row: u16, spans: Vec<Span>) -> Self {
        Self { row, spans }
    }

    /// Adds a span to this row.
    pub fn add_span(&mut self, span: Span) {
        self.spans.push(span);
    }
}

/// A snapshot represents the complete state of the terminal screen.
///
/// It contains both a hierarchical view (rows containing spans) and a
/// flat list of all spans for convenience.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    /// Rows in the snapshot, ordered from top to bottom.
    pub rows: Vec<Row>,

    /// Flat list of all spans across all rows.
    pub spans: Vec<Span>,

    /// YAML representation of the snapshot for human readability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaml: Option<String>,
}

impl Snapshot {
    /// Creates a new empty snapshot.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            spans: Vec::new(),
            yaml: None,
        }
    }

    /// Creates a snapshot from rows, automatically populating the flat spans list.
    pub fn from_rows(rows: Vec<Row>) -> Self {
        let spans: Vec<Span> = rows.iter().flat_map(|r| r.spans.clone()).collect();
        Self {
            rows,
            spans,
            yaml: None,
        }
    }

    /// Sets the YAML string representation.
    pub fn with_yaml(mut self, yaml: impl Into<String>) -> Self {
        self.yaml = Some(yaml.into());
        self
    }

    /// Gets a span by its reference ID.
    ///
    /// Returns the first span with a matching ref_id, or None if not found.
    pub fn get_by_ref(&self, ref_id: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.ref_id == ref_id)
    }

    /// Gets all spans containing the specified text.
    ///
    /// Returns a vector of references to spans whose text contains the
    /// given substring.
    pub fn get_by_text(&self, text: &str) -> Vec<&Span> {
        self.spans
            .iter()
            .filter(|s| s.text.contains(text))
            .collect()
    }

    /// Gets the first span containing the specified text.
    ///
    /// Returns the first span whose text contains the given substring,
    /// or None if no match is found.
    pub fn get_first_by_text(&self, text: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.text.contains(text))
    }

    /// Gets a span by exact text match.
    ///
    /// Returns the first span whose text exactly matches the given string,
    /// or None if no match is found.
    pub fn get_by_exact_text(&self, text: &str) -> Option<&Span> {
        self.spans.iter().find(|s| s.text == text)
    }

    /// Returns the total number of spans in this snapshot.
    pub fn span_count(&self) -> usize {
        self.spans.len()
    }

    /// Returns the number of rows in this snapshot.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Checks if the snapshot is empty (no spans).
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }
}

impl Default for Snapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a vt100 color to a string representation.
///
/// Returns None for default colors, otherwise returns a string like
/// "color0" for indexed colors or "#rrggbb" for RGB colors.
fn color_to_string(color: vt100::Color) -> Option<String> {
    match color {
        vt100::Color::Default => None,
        vt100::Color::Idx(i) => Some(format!("color{}", i)),
        vt100::Color::Rgb(r, g, b) => Some(format!("#{:02x}{:02x}{:02x}", r, g, b)),
    }
}

/// Check if two cells have the same styling attributes.
///
/// Returns true if both cells have identical bold, italic, underline,
/// inverse, foreground color, and background color attributes.
fn same_style(a: &vt100::Cell, b: &vt100::Cell) -> bool {
    a.bold() == b.bold()
        && a.italic() == b.italic()
        && a.underline() == b.underline()
        && a.inverse() == b.inverse()
        && a.fgcolor() == b.fgcolor()
        && a.bgcolor() == b.bgcolor()
}

/// Check if a cell is effectively empty (whitespace with default styling).
/// This is used to determine span boundaries.
fn is_empty_cell(cell: &vt100::Cell) -> bool {
    let contents = cell.contents();
    let is_blank = contents.is_empty() || contents == " ";
    let is_default_style = !cell.bold()
        && !cell.italic()
        && !cell.underline()
        && !cell.inverse()
        && cell.fgcolor() == vt100::Color::Default
        && cell.bgcolor() == vt100::Color::Default;

    is_blank && is_default_style
}

/// Build a Snapshot from a vt100 Screen.
///
/// This function extracts spans from the terminal screen by:
/// 1. Iterating through all rows and columns
/// 2. Grouping consecutive characters with identical styling into spans
/// 3. Assigning reference IDs like "s1", "s2", etc.
/// 4. Generating a YAML representation for human readability
///
/// Coordinates are 1-based (x=col+1, y=row+1) for user-facing output.
/// Empty cells and whitespace-only regions are skipped.
pub fn build_snapshot(screen: &Screen) -> Snapshot {
    let (num_rows, num_cols) = screen.size();
    let mut rows: Vec<Row> = Vec::new();
    let mut span_counter: usize = 0;

    for row_idx in 0..num_rows {
        let mut row_spans: Vec<Span> = Vec::new();
        let mut col_idx: u16 = 0;

        while col_idx < num_cols {
            // Get the current cell
            let cell = match screen.cell(row_idx, col_idx) {
                Some(c) => c,
                None => {
                    col_idx += 1;
                    continue;
                }
            };

            // Skip empty cells (whitespace with default styling)
            if is_empty_cell(cell) {
                col_idx += 1;
                continue;
            }

            // Start a new span
            let start_col = col_idx;
            let mut text = String::new();
            let first_cell = cell;

            // Collect consecutive cells with the same style
            // We continue through whitespace as long as:
            // 1. The whitespace has the same styling, OR
            // 2. The whitespace is followed by content with the same styling
            while col_idx < num_cols {
                let current_cell = match screen.cell(row_idx, col_idx) {
                    Some(c) => c,
                    None => break,
                };

                // Check if this cell has the same style as the first cell
                if !same_style(first_cell, current_cell) {
                    // Style changed - but if this is empty with default style,
                    // we might want to continue if there's more styled content ahead
                    if is_empty_cell(current_cell) {
                        // Look ahead to see if there's more content with our style
                        let mut found_continuation = false;
                        let mut lookahead = col_idx + 1;
                        while lookahead < num_cols {
                            if let Some(ahead_cell) = screen.cell(row_idx, lookahead) {
                                if is_empty_cell(ahead_cell) {
                                    lookahead += 1;
                                    continue;
                                }
                                // Found a non-empty cell
                                if same_style(first_cell, ahead_cell) {
                                    found_continuation = true;
                                }
                                break;
                            }
                            lookahead += 1;
                        }
                        if found_continuation {
                            // Include this whitespace and continue
                            let contents = current_cell.contents();
                            if contents.is_empty() {
                                text.push(' ');
                            } else {
                                text.push_str(&contents);
                            }
                            col_idx += 1;
                            continue;
                        }
                    }
                    // Style changed and no continuation found - stop the span
                    break;
                }

                let contents = current_cell.contents();
                if contents.is_empty() {
                    text.push(' ');
                } else {
                    text.push_str(&contents);
                }
                col_idx += 1;
            }

            // Trim trailing whitespace from the span text
            let trimmed_text = text.trim_end().to_string();
            if trimmed_text.is_empty() {
                continue;
            }

            // Calculate the actual width (number of cells this span occupies)
            let width = trimmed_text.chars().count() as u16;

            // Create the span with 1-based coordinates
            span_counter += 1;
            let ref_id = format!("s{}", span_counter);

            let mut span = Span::new(
                ref_id,
                trimmed_text,
                start_col + 1, // 1-based x coordinate
                row_idx + 1,   // 1-based y coordinate
                width,
            );

            // Add styling attributes (only if non-default)
            if first_cell.bold() {
                span.bold = Some(true);
            }
            if first_cell.italic() {
                span.italic = Some(true);
            }
            if first_cell.underline() {
                span.underline = Some(true);
            }
            if first_cell.inverse() {
                span.inverse = Some(true);
            }
            if let Some(fg) = color_to_string(first_cell.fgcolor()) {
                span.fg = Some(fg);
            }
            if let Some(bg) = color_to_string(first_cell.bgcolor()) {
                span.bg = Some(bg);
            }

            row_spans.push(span);
        }

        // Only add rows that have spans
        if !row_spans.is_empty() {
            rows.push(Row::with_spans(row_idx + 1, row_spans)); // 1-based row number
        }
    }

    // Generate the YAML representation
    let yaml = generate_yaml(&rows);

    // Create the snapshot
    let spans: Vec<Span> = rows.iter().flat_map(|r| r.spans.clone()).collect();
    Snapshot {
        rows,
        spans,
        yaml: Some(yaml),
    }
}

/// Generate a YAML-like representation of the rows and spans.
///
/// The format is:
/// ```yaml
/// - row 1:
///   - span "text" [bold] [ref=s1] (1,1)
/// ```
fn generate_yaml(rows: &[Row]) -> String {
    let mut result = String::new();

    for row in rows {
        result.push_str(&format!("- row {}:\n", row.row));

        for span in &row.spans {
            let mut attrs: Vec<String> = Vec::new();

            if span.bold == Some(true) {
                attrs.push("bold".to_string());
            }
            if span.italic == Some(true) {
                attrs.push("italic".to_string());
            }
            if span.underline == Some(true) {
                attrs.push("underline".to_string());
            }
            if span.inverse == Some(true) {
                attrs.push("inverse".to_string());
            }
            if let Some(ref fg) = span.fg {
                attrs.push(format!("fg={}", fg));
            }
            if let Some(ref bg) = span.bg {
                attrs.push(format!("bg={}", bg));
            }

            let attrs_str = if attrs.is_empty() {
                String::new()
            } else {
                format!(" [{}]", attrs.join(", "))
            };

            result.push_str(&format!(
                "  - span \"{}\"{}[ref={}] ({},{})\n",
                escape_yaml_string(&span.text),
                attrs_str,
                span.ref_id,
                span.x,
                span.y
            ));
        }
    }

    result
}

/// Escape special characters in a string for YAML output.
fn escape_yaml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// A screenshot represents a PNG image of the terminal screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    /// Base64-encoded PNG image data.
    pub data: String,
    /// Image format (always "png").
    pub format: String,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Render terminal screen to PNG image.
///
/// This function creates a simple visual representation of the terminal
/// by rendering non-empty cells as colored rectangles. The rendering uses
/// a fixed character size of 10x20 pixels.
pub fn render_screenshot(screen: &Screen) -> Screenshot {
    let (rows, cols) = screen.size();
    let char_width = 10u32;
    let char_height = 20u32;
    let width = cols as u32 * char_width;
    let height = rows as u32 * char_height;

    let mut img: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 255]));

    // Simple rendering: fill rectangles for non-empty cells
    for row in 0..rows {
        for col in 0..cols {
            if let Some(cell) = screen.cell(row, col) {
                let contents = cell.contents();
                if !contents.is_empty() && contents != " " {
                    let px = col as u32 * char_width;
                    let py = row as u32 * char_height;

                    let fg = match cell.fgcolor() {
                        vt100::Color::Default => Rgba([255, 255, 255, 255]),
                        vt100::Color::Idx(i) => ansi_to_rgba(i),
                        vt100::Color::Rgb(r, g, b) => Rgba([r, g, b, 255]),
                    };

                    // Fill cell area
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

    // Encode to PNG
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

/// Convert ANSI color index to RGBA.
fn ansi_to_rgba(idx: u8) -> Rgba<u8> {
    match idx {
        0 => Rgba([0, 0, 0, 255]),
        1 => Rgba([205, 0, 0, 255]),
        2 => Rgba([0, 205, 0, 255]),
        3 => Rgba([205, 205, 0, 255]),
        4 => Rgba([0, 0, 238, 255]),
        5 => Rgba([205, 0, 205, 255]),
        6 => Rgba([0, 205, 205, 255]),
        7 => Rgba([229, 229, 229, 255]),
        8 => Rgba([127, 127, 127, 255]),
        9 => Rgba([255, 0, 0, 255]),
        10 => Rgba([0, 255, 0, 255]),
        11 => Rgba([255, 255, 0, 255]),
        12 => Rgba([92, 92, 255, 255]),
        13 => Rgba([255, 0, 255, 255]),
        14 => Rgba([0, 255, 255, 255]),
        15 => Rgba([255, 255, 255, 255]),
        _ => Rgba([200, 200, 200, 255]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_span_creation() {
        let span = Span::new("ref1", "Hello", 0, 0, 5);
        assert_eq!(span.ref_id, "ref1");
        assert_eq!(span.text, "Hello");
        assert_eq!(span.x, 0);
        assert_eq!(span.y, 0);
        assert_eq!(span.width, 5);
        assert!(span.bold.is_none());
        assert!(span.fg.is_none());
    }

    #[test]
    fn test_span_with_styling() {
        let span = Span::new("ref1", "Bold", 0, 0, 4)
            .with_bold(true)
            .with_fg("red")
            .with_bg("white");

        assert_eq!(span.bold, Some(true));
        assert_eq!(span.fg, Some("red".to_string()));
        assert_eq!(span.bg, Some("white".to_string()));
    }

    #[test]
    fn test_row_creation() {
        let mut row = Row::new(0);
        row.add_span(Span::new("ref1", "Hello", 0, 0, 5));
        row.add_span(Span::new("ref2", " World", 5, 0, 6));

        assert_eq!(row.row, 0);
        assert_eq!(row.spans.len(), 2);
    }

    #[test]
    fn test_snapshot_from_rows() {
        let rows = vec![
            Row::with_spans(0, vec![Span::new("ref1", "Line1", 0, 0, 5)]),
            Row::with_spans(1, vec![Span::new("ref2", "Line2", 0, 1, 5)]),
        ];

        let snapshot = Snapshot::from_rows(rows);
        assert_eq!(snapshot.row_count(), 2);
        assert_eq!(snapshot.span_count(), 2);
    }

    #[test]
    fn test_get_by_ref() {
        let rows = vec![Row::with_spans(
            0,
            vec![
                Span::new("ref1", "Hello", 0, 0, 5),
                Span::new("ref2", "World", 5, 0, 5),
            ],
        )];

        let snapshot = Snapshot::from_rows(rows);

        let found = snapshot.get_by_ref("ref2");
        assert!(found.is_some());
        assert_eq!(found.unwrap().text, "World");

        let not_found = snapshot.get_by_ref("ref999");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_by_text() {
        let rows = vec![Row::with_spans(
            0,
            vec![
                Span::new("ref1", "Hello World", 0, 0, 11),
                Span::new("ref2", "Goodbye World", 0, 1, 13),
            ],
        )];

        let snapshot = Snapshot::from_rows(rows);

        let matches = snapshot.get_by_text("World");
        assert_eq!(matches.len(), 2);

        let hello_matches = snapshot.get_by_text("Hello");
        assert_eq!(hello_matches.len(), 1);
        assert_eq!(hello_matches[0].ref_id, "ref1");
    }

    #[test]
    fn test_serialization_skip_none() {
        let span = Span::new("ref1", "Test", 0, 0, 4);
        let json = serde_json::to_string(&span).unwrap();

        // Optional fields with None should be skipped
        assert!(!json.contains("bold"));
        assert!(!json.contains("italic"));
        assert!(!json.contains("fg"));
        assert!(!json.contains("bg"));
    }

    #[test]
    fn test_serialization_with_values() {
        let span = Span::new("ref1", "Test", 0, 0, 4).with_bold(true);
        let json = serde_json::to_string(&span).unwrap();

        // Bold should be present since it has a value
        assert!(json.contains("bold"));
        assert!(json.contains("true"));
    }

    #[test]
    fn test_build_snapshot_basic() {
        // Create a parser and write some text to it
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Hello World");

        let snapshot = build_snapshot(parser.screen());

        // Should have exactly one span (text with same style grouped together)
        assert!(!snapshot.is_empty());
        assert_eq!(snapshot.span_count(), 1);

        // Check the span content
        let span = &snapshot.spans[0];
        assert_eq!(span.text, "Hello World");
        assert_eq!(span.ref_id, "s1");
        assert_eq!(span.x, 1); // 1-based
        assert_eq!(span.y, 1); // 1-based
        assert_eq!(span.width, 11);

        // Should have YAML output
        assert!(snapshot.yaml.is_some());
        let yaml = snapshot.yaml.as_ref().unwrap();
        assert!(yaml.contains("row 1"));
        assert!(yaml.contains("Hello World"));
        assert!(yaml.contains("ref=s1"));
    }

    #[test]
    fn test_build_snapshot_with_styling() {
        // Create a parser and write styled text
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[1m = bold on, ESC[0m = reset
        parser.process(b"\x1b[1mBold\x1b[0m Normal");

        let snapshot = build_snapshot(parser.screen());

        // Should have two spans (bold and normal)
        assert_eq!(snapshot.span_count(), 2);

        // First span should be bold
        let bold_span = &snapshot.spans[0];
        assert_eq!(bold_span.text, "Bold");
        assert_eq!(bold_span.bold, Some(true));

        // Second span should be normal
        let normal_span = &snapshot.spans[1];
        assert_eq!(normal_span.text, "Normal");
        assert!(normal_span.bold.is_none());
    }

    #[test]
    fn test_build_snapshot_multiple_rows() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        parser.process(b"Line 1\r\nLine 2\r\nLine 3");

        let snapshot = build_snapshot(parser.screen());

        assert_eq!(snapshot.row_count(), 3);
        assert_eq!(snapshot.span_count(), 3);

        // Check each row
        assert_eq!(snapshot.rows[0].row, 1);
        assert_eq!(snapshot.rows[0].spans[0].text, "Line 1");

        assert_eq!(snapshot.rows[1].row, 2);
        assert_eq!(snapshot.rows[1].spans[0].text, "Line 2");

        assert_eq!(snapshot.rows[2].row, 3);
        assert_eq!(snapshot.rows[2].spans[0].text, "Line 3");
    }

    #[test]
    fn test_build_snapshot_with_colors() {
        let mut parser = vt100::Parser::new(24, 80, 0);
        // ESC[31m = red foreground
        parser.process(b"\x1b[31mRed Text\x1b[0m");

        let snapshot = build_snapshot(parser.screen());

        assert_eq!(snapshot.span_count(), 1);
        let span = &snapshot.spans[0];
        assert_eq!(span.text, "Red Text");
        // Color 1 is red in standard ANSI
        assert_eq!(span.fg, Some("color1".to_string()));
    }

    #[test]
    fn test_build_snapshot_empty_screen() {
        let parser = vt100::Parser::new(24, 80, 0);
        let snapshot = build_snapshot(parser.screen());

        assert!(snapshot.is_empty());
        assert_eq!(snapshot.row_count(), 0);
        assert_eq!(snapshot.span_count(), 0);
    }

    #[test]
    fn test_color_to_string() {
        assert_eq!(color_to_string(vt100::Color::Default), None);
        assert_eq!(
            color_to_string(vt100::Color::Idx(1)),
            Some("color1".to_string())
        );
        assert_eq!(
            color_to_string(vt100::Color::Idx(255)),
            Some("color255".to_string())
        );
        assert_eq!(
            color_to_string(vt100::Color::Rgb(255, 128, 0)),
            Some("#ff8000".to_string())
        );
    }

    #[test]
    fn test_escape_yaml_string() {
        assert_eq!(escape_yaml_string("hello"), "hello");
        assert_eq!(escape_yaml_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_yaml_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_yaml_string("tab\there"), "tab\\there");
    }

    #[test]
    fn test_generate_yaml_format() {
        let rows = vec![Row::with_spans(
            1,
            vec![Span::new("s1", "Hello", 1, 1, 5).with_bold(true)],
        )];

        let yaml = generate_yaml(&rows);

        assert!(yaml.contains("- row 1:"));
        assert!(yaml.contains("span \"Hello\""));
        assert!(yaml.contains("[bold]"));
        assert!(yaml.contains("[ref=s1]"));
        assert!(yaml.contains("(1,1)"));
    }
}
