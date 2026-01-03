//! Terminal emulator wrapper using wezterm-term

use parking_lot::Mutex;
use std::sync::Arc;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

/// Configuration for wezterm terminal
#[derive(Debug, Clone)]
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
        // Verify no panic - actual text extraction tested later
    }

    #[test]
    fn test_terminal_resize() {
        let term = TuiTerminal::new(24, 80, 500);
        term.resize(40, 120);
        assert_eq!(term.size(), (40, 120));
    }
}
