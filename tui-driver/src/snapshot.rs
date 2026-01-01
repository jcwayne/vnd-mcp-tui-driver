//! Snapshot types for capturing terminal state.
//!
//! This module provides types for representing a snapshot of the terminal
//! screen, including individual spans with styling information, rows of
//! spans, and the complete snapshot structure.

use serde::{Deserialize, Serialize};

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
    pub fn new(ref_id: impl Into<String>, text: impl Into<String>, x: u16, y: u16, width: u16) -> Self {
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
        self.spans.iter().filter(|s| s.text.contains(text)).collect()
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
            Row::with_spans(0, vec![
                Span::new("ref1", "Line1", 0, 0, 5),
            ]),
            Row::with_spans(1, vec![
                Span::new("ref2", "Line2", 0, 1, 5),
            ]),
        ];

        let snapshot = Snapshot::from_rows(rows);
        assert_eq!(snapshot.row_count(), 2);
        assert_eq!(snapshot.span_count(), 2);
    }

    #[test]
    fn test_get_by_ref() {
        let rows = vec![
            Row::with_spans(0, vec![
                Span::new("ref1", "Hello", 0, 0, 5),
                Span::new("ref2", "World", 5, 0, 5),
            ]),
        ];

        let snapshot = Snapshot::from_rows(rows);

        let found = snapshot.get_by_ref("ref2");
        assert!(found.is_some());
        assert_eq!(found.unwrap().text, "World");

        let not_found = snapshot.get_by_ref("ref999");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_by_text() {
        let rows = vec![
            Row::with_spans(0, vec![
                Span::new("ref1", "Hello World", 0, 0, 11),
                Span::new("ref2", "Goodbye World", 0, 1, 13),
            ]),
        ];

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
}
