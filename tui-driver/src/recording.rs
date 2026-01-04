//! Recording module for capturing terminal sessions in asciicast v3 format.
//!
//! This module provides functionality to record terminal sessions to `.cast` files
//! that can be played back with asciinema or other compatible players.
//!
//! # Asciicast v3 Format
//!
//! The asciicast v3 format is a newline-delimited JSON format for recording terminal sessions.
//! Each file consists of:
//!
//! 1. **Header line**: A JSON object containing metadata about the recording:
//!    - `version`: Always `3` for asciicast v3
//!    - `term`: Object with `cols` and `rows` for terminal dimensions
//!    - `timestamp`: Unix timestamp when the recording started
//!    - `command`: The command that was executed
//!
//! 2. **Event lines**: JSON arrays in the format `[interval, "type", "data"]`:
//!    - `interval`: Time since the last event in seconds (floating point)
//!    - `type`: Event type character:
//!      - `"o"` - Output: Data written to the terminal
//!      - `"i"` - Input: Data typed by the user (optional)
//!      - `"r"` - Resize: Terminal dimension change as "COLSxROWS"
//!      - `"x"` - Exit: Process termination with exit code
//!    - `data`: Event-specific data (string)
//!
//! # Example Recording File
//!
//! ```json
//! {"version":3,"term":{"cols":80,"rows":24},"timestamp":1704384000,"command":"bash"}
//! [0.5,"o","$ "]
//! [0.1,"i","ls\n"]
//! [0.0,"o","ls\n"]
//! [0.2,"o","file1.txt  file2.txt\n"]
//! [0.1,"o","$ "]
//! [1.0,"x","0"]
//! ```
//!
//! # Usage
//!
//! Recording is enabled through [`RecordingOptions`] when launching a TUI session:
//!
//! ```ignore
//! use tui_driver::{LaunchOptions, RecordingOptions};
//!
//! let options = LaunchOptions::new("bash")
//!     .recording(RecordingOptions::new("/tmp/session.cast"));
//!
//! // Session will be recorded automatically
//! let driver = TuiDriver::launch(options).await?;
//! ```
//!
//! # Playback
//!
//! Recordings can be played back using the asciinema CLI:
//!
//! ```bash
//! asciinema play /tmp/session.cast
//! ```

use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::error::{Result, TuiError};

/// Configuration options for session recording.
///
/// This struct controls whether and how a TUI session is recorded.
/// Recording captures terminal output, resize events, and optionally input events
/// to an asciicast v3 format file.
///
/// # Example
///
/// ```
/// use tui_driver::RecordingOptions;
///
/// // Basic recording (output only)
/// let opts = RecordingOptions::new("/tmp/session.cast");
///
/// // Recording with input events
/// let opts_with_input = RecordingOptions::new("/tmp/session.cast")
///     .with_input(true);
/// ```
#[derive(Debug, Clone, Default)]
pub struct RecordingOptions {
    /// Whether recording is enabled.
    ///
    /// When `true`, terminal events will be written to the output file.
    /// When `false`, no recording occurs (useful for conditional recording).
    pub enabled: bool,

    /// Path to write the recording file.
    ///
    /// The file will be created if it doesn't exist, or overwritten if it does.
    /// It is recommended to use the `.cast` extension for compatibility with
    /// asciinema and other players.
    pub output_path: String,

    /// Whether to include input events in the recording.
    ///
    /// When `true`, keystrokes and text input are recorded as `"i"` events.
    /// When `false` (default), only output, resize, and exit events are recorded.
    ///
    /// Note: Including input may expose sensitive information such as passwords.
    pub include_input: bool,
}

impl RecordingOptions {
    /// Creates new recording options with recording enabled.
    ///
    /// By default, input recording is disabled. Use [`with_input`](Self::with_input)
    /// to enable it.
    ///
    /// # Arguments
    ///
    /// * `output_path` - Path where the recording file will be written.
    ///
    /// # Example
    ///
    /// ```
    /// use tui_driver::RecordingOptions;
    ///
    /// let opts = RecordingOptions::new("/tmp/my-session.cast");
    /// assert!(opts.enabled);
    /// assert!(!opts.include_input);
    /// ```
    pub fn new(output_path: impl Into<String>) -> Self {
        Self {
            enabled: true,
            output_path: output_path.into(),
            include_input: false,
        }
    }

    /// Sets whether to include input events in the recording.
    ///
    /// This is a builder method that returns `self` for method chaining.
    ///
    /// # Arguments
    ///
    /// * `include_input` - If `true`, input events will be recorded.
    ///
    /// # Example
    ///
    /// ```
    /// use tui_driver::RecordingOptions;
    ///
    /// let opts = RecordingOptions::new("/tmp/session.cast")
    ///     .with_input(true);
    /// assert!(opts.include_input);
    /// ```
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
/// The `Recorder` captures terminal events and writes them to a file in the
/// asciicast v3 format. It handles the file creation, header writing, and
/// event serialization.
///
/// # Event Types
///
/// The recorder supports four event types:
/// - **Output** (`"o"`): Terminal output data, recorded via [`record_output`](Self::record_output)
/// - **Input** (`"i"`): User input data, recorded via [`record_input`](Self::record_input) (optional)
/// - **Resize** (`"r"`): Terminal dimension changes, recorded via [`record_resize`](Self::record_resize)
/// - **Exit** (`"x"`): Process termination, recorded via [`record_exit`](Self::record_exit)
///
/// # Timing
///
/// Events are timestamped with the interval (in seconds) since the last event.
/// The first event's interval is measured from the recorder's creation time.
///
/// # Thread Safety
///
/// `Recorder` is not thread-safe. It should only be accessed from a single thread
/// or protected by external synchronization.
///
/// # Example
///
/// ```ignore
/// use tui_driver::recording::Recorder;
///
/// let mut recorder = Recorder::new("/tmp/session.cast", 80, 24, "bash", false)?;
/// recorder.record_output("Hello, world!\n");
/// recorder.record_resize(120, 40);
/// recorder.record_exit(0);
/// ```
pub struct Recorder {
    /// Buffered file writer for efficient I/O.
    file: BufWriter<File>,
    /// Timestamp of the last recorded event, used to calculate intervals.
    last_event_time: Instant,
    /// Whether to record input events (controlled by RecordingOptions).
    include_input: bool,
}

impl Recorder {
    /// Creates a new recorder and writes the asciicast header.
    ///
    /// This constructor creates the output file, writes the JSON header line,
    /// and initializes the recorder for capturing events.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the output file. Will be created or overwritten.
    /// * `cols` - Initial terminal width in columns.
    /// * `rows` - Initial terminal height in rows.
    /// * `command` - The command being recorded (stored in header).
    /// * `include_input` - Whether to record input events via [`record_input`](Self::record_input).
    ///
    /// # Returns
    ///
    /// A new `Recorder` instance, or an error if the file cannot be created.
    ///
    /// # Errors
    ///
    /// Returns [`TuiError::IoError`] if:
    /// - The output file cannot be created (permissions, invalid path, etc.)
    /// - The header cannot be serialized or written
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

    /// Calculates the time interval since the last event.
    ///
    /// Updates the internal timestamp and returns the elapsed time in seconds.
    /// This is used internally to timestamp each event.
    fn interval(&mut self) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_event_time);
        self.last_event_time = now;
        elapsed.as_secs_f64()
    }

    /// Writes an event to the recording file in asciicast format.
    ///
    /// Events are written as JSON arrays: `[interval, "type", "data"]`
    /// The data is properly escaped for JSON encoding.
    fn write_event(&mut self, interval: f64, event_type: &str, data: &str) {
        // Format: [interval,"type","data"]
        // We need to escape the data for JSON
        let escaped_data = serde_json::to_string(data).unwrap_or_else(|_| "\"\"".to_string());

        // Write the event line
        let _ = writeln!(self.file, "[{},\"{}\",{}]", interval, event_type, escaped_data);
    }

    /// Records terminal output data.
    ///
    /// This records data that is written to the terminal (the output stream).
    /// Output events are recorded as `"o"` type events.
    ///
    /// # Arguments
    ///
    /// * `data` - The output data to record. May contain ANSI escape sequences.
    pub fn record_output(&mut self, data: &str) {
        let interval = self.interval();
        self.write_event(interval, "o", data);
    }

    /// Records user input data.
    ///
    /// This records data that is sent to the terminal (keystrokes, text input).
    /// Input events are recorded as `"i"` type events.
    ///
    /// **Note**: This method only writes to the file if `include_input` was set
    /// to `true` when creating the recorder. Otherwise, it is a no-op.
    ///
    /// # Arguments
    ///
    /// * `data` - The input data to record.
    ///
    /// # Security Consideration
    ///
    /// Input data may contain sensitive information such as passwords.
    /// Only enable input recording when necessary.
    pub fn record_input(&mut self, data: &str) {
        if self.include_input {
            let interval = self.interval();
            self.write_event(interval, "i", data);
        }
    }

    /// Records a terminal resize event.
    ///
    /// This records when the terminal dimensions change. Resize events are
    /// recorded as `"r"` type events with data in the format `"COLSxROWS"`.
    ///
    /// # Arguments
    ///
    /// * `cols` - New terminal width in columns.
    /// * `rows` - New terminal height in rows.
    pub fn record_resize(&mut self, cols: u16, rows: u16) {
        let interval = self.interval();
        let size_str = format!("{}x{}", cols, rows);
        self.write_event(interval, "r", &size_str);
    }

    /// Records a process exit event.
    ///
    /// This records when the terminal process terminates. Exit events are
    /// recorded as `"x"` type events with the exit code as the data.
    ///
    /// This method also flushes the internal buffer to ensure all data is
    /// written to disk before the file is closed.
    ///
    /// # Arguments
    ///
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
