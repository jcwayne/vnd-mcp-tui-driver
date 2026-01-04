//! Recording module for capturing terminal sessions in asciicast v3 format.
//!
//! This module provides functionality to record terminal sessions to .cast files
//! that can be played back with asciinema or other compatible players.

use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::error::{Result, TuiError};

/// Configuration options for session recording.
#[derive(Debug, Clone, Default)]
pub struct RecordingOptions {
    /// Whether recording is enabled.
    pub enabled: bool,
    /// Path to write the recording file.
    pub output_path: String,
    /// Whether to include input events in the recording.
    pub include_input: bool,
}

impl RecordingOptions {
    /// Create new recording options with recording enabled.
    pub fn new(output_path: impl Into<String>) -> Self {
        Self {
            enabled: true,
            output_path: output_path.into(),
            include_input: false,
        }
    }

    /// Enable or disable input recording.
    pub fn with_input(mut self, include_input: bool) -> Self {
        self.include_input = include_input;
        self
    }
}

/// Asciicast v3 header format.
#[derive(Debug, Serialize)]
struct AsciicastHeader {
    version: u8,
    term: TermInfo,
    timestamp: u64,
    command: String,
}

/// Terminal information in the header.
#[derive(Debug, Serialize)]
struct TermInfo {
    cols: u16,
    rows: u16,
}

/// Recorder for capturing terminal sessions in asciicast v3 format.
///
/// The recorder captures output, input (optional), resize, and exit events
/// and writes them to a file in the asciicast v3 format.
pub struct Recorder {
    /// Buffered file writer.
    file: BufWriter<File>,
    /// Time of the last recorded event (for calculating intervals).
    last_event_time: Instant,
    /// Whether to record input events.
    include_input: bool,
}

impl Recorder {
    /// Create a new recorder.
    ///
    /// # Arguments
    /// * `path` - Path to the output file.
    /// * `cols` - Terminal width in columns.
    /// * `rows` - Terminal height in rows.
    /// * `command` - The command being recorded.
    /// * `include_input` - Whether to record input events.
    ///
    /// # Returns
    /// A new Recorder instance or an error if the file cannot be created.
    pub fn new(
        path: &str,
        cols: u16,
        rows: u16,
        command: &str,
        include_input: bool,
    ) -> Result<Self> {
        let file = File::create(path).map_err(|e| {
            TuiError::IoError(std::io::Error::new(
                e.kind(),
                format!("Failed to create recording file '{}': {}", path, e),
            ))
        })?;
        let mut file = BufWriter::new(file);

        // Get current timestamp
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Write header
        let header = AsciicastHeader {
            version: 3,
            term: TermInfo { cols, rows },
            timestamp,
            command: command.to_string(),
        };

        let header_json = serde_json::to_string(&header).map_err(|e| {
            TuiError::IoError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize header: {}", e),
            ))
        })?;

        writeln!(file, "{}", header_json)?;

        Ok(Self {
            file,
            last_event_time: Instant::now(),
            include_input,
        })
    }

    /// Calculate the interval since the last event in seconds.
    fn interval(&mut self) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_event_time);
        self.last_event_time = now;
        elapsed.as_secs_f64()
    }

    /// Write an event to the recording file.
    fn write_event(&mut self, interval: f64, event_type: &str, data: &str) {
        // Format: [interval,"type","data"]
        // We need to escape the data for JSON
        let escaped_data = serde_json::to_string(data).unwrap_or_else(|_| "\"\"".to_string());

        // Write the event line
        let _ = writeln!(self.file, "[{},\"{}\",{}]", interval, event_type, escaped_data);
    }

    /// Record output data.
    ///
    /// # Arguments
    /// * `data` - The output data to record.
    pub fn record_output(&mut self, data: &str) {
        let interval = self.interval();
        self.write_event(interval, "o", data);
    }

    /// Record input data.
    ///
    /// Only writes if `include_input` was set to true when creating the recorder.
    ///
    /// # Arguments
    /// * `data` - The input data to record.
    pub fn record_input(&mut self, data: &str) {
        if self.include_input {
            let interval = self.interval();
            self.write_event(interval, "i", data);
        }
    }

    /// Record a resize event.
    ///
    /// # Arguments
    /// * `cols` - New terminal width.
    /// * `rows` - New terminal height.
    pub fn record_resize(&mut self, cols: u16, rows: u16) {
        let interval = self.interval();
        let size_str = format!("{}x{}", cols, rows);
        self.write_event(interval, "r", &size_str);
    }

    /// Record an exit event.
    ///
    /// # Arguments
    /// * `code` - The exit code of the process.
    pub fn record_exit(&mut self, code: i32) {
        let interval = self.interval();
        self.write_event(interval, "x", &code.to_string());
        // Flush the buffer to ensure all data is written
        let _ = self.file.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_recording_options_new() {
        let opts = RecordingOptions::new("/tmp/test.cast");
        assert!(opts.enabled);
        assert_eq!(opts.output_path, "/tmp/test.cast");
        assert!(!opts.include_input);
    }

    #[test]
    fn test_recording_options_with_input() {
        let opts = RecordingOptions::new("/tmp/test.cast").with_input(true);
        assert!(opts.include_input);
    }

    #[test]
    fn test_recorder_creates_file_with_header() {
        let path = "/tmp/test_recorder_header.cast";

        // Create recorder
        let recorder = Recorder::new(path, 80, 24, "bash", false);
        assert!(recorder.is_ok());

        // Drop recorder to flush
        drop(recorder);

        // Read and verify file
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // Verify header
        assert!(!lines.is_empty());
        let header: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["version"], 3);
        assert_eq!(header["term"]["cols"], 80);
        assert_eq!(header["term"]["rows"], 24);
        assert_eq!(header["command"], "bash");

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_output_event() {
        let path = "/tmp/test_recorder_output.cast";

        // Create recorder and record output
        let mut recorder = Recorder::new(path, 80, 24, "bash", false).unwrap();
        recorder.record_output("Hello, world!");
        drop(recorder);

        // Read and verify
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert!(lines.len() >= 2);
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[1], "o");
        assert_eq!(event[2], "Hello, world!");

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_input_event_when_enabled() {
        let path = "/tmp/test_recorder_input.cast";

        // Create recorder with input enabled
        let mut recorder = Recorder::new(path, 80, 24, "bash", true).unwrap();
        recorder.record_input("ls\n");
        drop(recorder);

        // Read and verify
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert!(lines.len() >= 2);
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[1], "i");
        assert_eq!(event[2], "ls\n");

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_input_event_when_disabled() {
        let path = "/tmp/test_recorder_input_disabled.cast";

        // Create recorder with input disabled
        let mut recorder = Recorder::new(path, 80, 24, "bash", false).unwrap();
        recorder.record_input("ls\n");
        drop(recorder);

        // Read and verify - should only have header
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 1); // Only header

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_resize_event() {
        let path = "/tmp/test_recorder_resize.cast";

        // Create recorder and record resize
        let mut recorder = Recorder::new(path, 80, 24, "bash", false).unwrap();
        recorder.record_resize(120, 40);
        drop(recorder);

        // Read and verify
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert!(lines.len() >= 2);
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[1], "r");
        assert_eq!(event[2], "120x40");

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_exit_event() {
        let path = "/tmp/test_recorder_exit.cast";

        // Create recorder and record exit
        let mut recorder = Recorder::new(path, 80, 24, "bash", false).unwrap();
        recorder.record_exit(0);
        drop(recorder);

        // Read and verify
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert!(lines.len() >= 2);
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[1], "x");
        assert_eq!(event[2], "0");

        // Cleanup
        fs::remove_file(path).ok();
    }

    #[test]
    fn test_recorder_escape_sequences() {
        let path = "/tmp/test_recorder_escape.cast";

        // Create recorder and record output with escape sequences
        let mut recorder = Recorder::new(path, 80, 24, "bash", false).unwrap();
        recorder.record_output("\x1b[32mGreen\x1b[0m");
        drop(recorder);

        // Read and verify - escape sequences should be preserved
        let content = fs::read_to_string(path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert!(lines.len() >= 2);
        let event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(event[1], "o");
        assert_eq!(event[2], "\x1b[32mGreen\x1b[0m");

        // Cleanup
        fs::remove_file(path).ok();
    }
}
