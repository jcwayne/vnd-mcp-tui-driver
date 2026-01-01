//! Key mapping module for converting key names to ANSI escape sequences.

use crate::error::{Result, TuiError};

/// Represents keyboard keys that can be sent to a terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    // Special keys
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Space,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Navigation keys
    Home,
    End,
    PageUp,
    PageDown,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Modifier combinations
    Ctrl(char),
    Alt(char),

    // Regular character
    Char(char),
}

impl Key {
    /// Converts the key to its ANSI escape sequence representation.
    ///
    /// Returns the bytes that should be written to the terminal to simulate
    /// pressing this key.
    pub fn to_escape_sequence(&self) -> Vec<u8> {
        match self {
            // Special keys
            Key::Enter => vec![b'\r'],
            Key::Tab => vec![b'\t'],
            Key::Escape => vec![0x1b],
            Key::Backspace => vec![0x7f],
            Key::Delete => b"\x1b[3~".to_vec(),
            Key::Insert => b"\x1b[2~".to_vec(),
            Key::Space => vec![b' '],

            // Arrow keys (CSI sequences)
            Key::Up => b"\x1b[A".to_vec(),
            Key::Down => b"\x1b[B".to_vec(),
            Key::Right => b"\x1b[C".to_vec(),
            Key::Left => b"\x1b[D".to_vec(),

            // Navigation keys
            Key::Home => b"\x1b[H".to_vec(),
            Key::End => b"\x1b[F".to_vec(),
            Key::PageUp => b"\x1b[5~".to_vec(),
            Key::PageDown => b"\x1b[6~".to_vec(),

            // Function keys
            Key::F1 => b"\x1bOP".to_vec(),
            Key::F2 => b"\x1bOQ".to_vec(),
            Key::F3 => b"\x1bOR".to_vec(),
            Key::F4 => b"\x1bOS".to_vec(),
            Key::F5 => b"\x1b[15~".to_vec(),
            Key::F6 => b"\x1b[17~".to_vec(),
            Key::F7 => b"\x1b[18~".to_vec(),
            Key::F8 => b"\x1b[19~".to_vec(),
            Key::F9 => b"\x1b[20~".to_vec(),
            Key::F10 => b"\x1b[21~".to_vec(),
            Key::F11 => b"\x1b[23~".to_vec(),
            Key::F12 => b"\x1b[24~".to_vec(),

            // Ctrl combinations: Ctrl+A = 0x01, Ctrl+B = 0x02, ..., Ctrl+Z = 0x1A
            Key::Ctrl(c) => {
                let c_lower = c.to_ascii_lowercase();
                if c_lower.is_ascii_lowercase() {
                    // Ctrl+a through Ctrl+z map to 0x01 through 0x1A
                    vec![(c_lower as u8) - b'a' + 1]
                } else {
                    // For other characters, just return as-is (best effort)
                    vec![*c as u8]
                }
            }

            // Alt combinations: ESC followed by the character
            Key::Alt(c) => {
                vec![0x1b, *c as u8]
            }

            // Regular character
            Key::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
    }

    /// Parses a string representation of a key into a Key enum.
    ///
    /// Supported formats:
    /// - Special keys: "Enter", "Tab", "Escape", "Backspace", "Delete", "Insert", "Space"
    /// - Arrow keys: "Up", "Down", "Left", "Right", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"
    /// - Navigation: "Home", "End", "PageUp", "PageDown"
    /// - Function keys: "F1" through "F12"
    /// - Ctrl combinations: "Ctrl+c", "Ctrl+C" (case insensitive for the character)
    /// - Alt combinations: "Alt+x", "Alt+X"
    /// - Single characters: "a", "A", "1", etc.
    pub fn parse(s: &str) -> Result<Self> {
        let s_trimmed = s.trim();

        // Check for modifier combinations first
        if let Some(rest) = s_trimmed.strip_prefix("Ctrl+").or_else(|| s_trimmed.strip_prefix("ctrl+")) {
            let chars: Vec<char> = rest.chars().collect();
            if chars.len() == 1 {
                return Ok(Key::Ctrl(chars[0]));
            } else {
                return Err(TuiError::InvalidKey(format!(
                    "Invalid Ctrl combination: {}",
                    s_trimmed
                )));
            }
        }

        if let Some(rest) = s_trimmed.strip_prefix("Alt+").or_else(|| s_trimmed.strip_prefix("alt+")) {
            let chars: Vec<char> = rest.chars().collect();
            if chars.len() == 1 {
                return Ok(Key::Alt(chars[0]));
            } else {
                return Err(TuiError::InvalidKey(format!(
                    "Invalid Alt combination: {}",
                    s_trimmed
                )));
            }
        }

        // Match special keys (case insensitive)
        match s_trimmed.to_lowercase().as_str() {
            "enter" | "return" => Ok(Key::Enter),
            "tab" => Ok(Key::Tab),
            "escape" | "esc" => Ok(Key::Escape),
            "backspace" => Ok(Key::Backspace),
            "delete" | "del" => Ok(Key::Delete),
            "insert" | "ins" => Ok(Key::Insert),
            "space" => Ok(Key::Space),

            // Arrow keys
            "up" | "arrowup" => Ok(Key::Up),
            "down" | "arrowdown" => Ok(Key::Down),
            "left" | "arrowleft" => Ok(Key::Left),
            "right" | "arrowright" => Ok(Key::Right),

            // Navigation keys
            "home" => Ok(Key::Home),
            "end" => Ok(Key::End),
            "pageup" | "pgup" => Ok(Key::PageUp),
            "pagedown" | "pgdown" | "pgdn" => Ok(Key::PageDown),

            // Function keys
            "f1" => Ok(Key::F1),
            "f2" => Ok(Key::F2),
            "f3" => Ok(Key::F3),
            "f4" => Ok(Key::F4),
            "f5" => Ok(Key::F5),
            "f6" => Ok(Key::F6),
            "f7" => Ok(Key::F7),
            "f8" => Ok(Key::F8),
            "f9" => Ok(Key::F9),
            "f10" => Ok(Key::F10),
            "f11" => Ok(Key::F11),
            "f12" => Ok(Key::F12),

            _ => {
                // Check if it's a single character
                let chars: Vec<char> = s_trimmed.chars().collect();
                if chars.len() == 1 {
                    Ok(Key::Char(chars[0]))
                } else {
                    Err(TuiError::InvalidKey(format!("Unknown key: {}", s_trimmed)))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(Key::parse("Enter").unwrap(), Key::Enter);
        assert_eq!(Key::parse("enter").unwrap(), Key::Enter);
        assert_eq!(Key::parse("Tab").unwrap(), Key::Tab);
        assert_eq!(Key::parse("Escape").unwrap(), Key::Escape);
        assert_eq!(Key::parse("esc").unwrap(), Key::Escape);
        assert_eq!(Key::parse("Backspace").unwrap(), Key::Backspace);
        assert_eq!(Key::parse("Delete").unwrap(), Key::Delete);
        assert_eq!(Key::parse("Insert").unwrap(), Key::Insert);
        assert_eq!(Key::parse("Space").unwrap(), Key::Space);
    }

    #[test]
    fn test_parse_arrow_keys() {
        assert_eq!(Key::parse("Up").unwrap(), Key::Up);
        assert_eq!(Key::parse("ArrowUp").unwrap(), Key::Up);
        assert_eq!(Key::parse("Down").unwrap(), Key::Down);
        assert_eq!(Key::parse("ArrowDown").unwrap(), Key::Down);
        assert_eq!(Key::parse("Left").unwrap(), Key::Left);
        assert_eq!(Key::parse("ArrowLeft").unwrap(), Key::Left);
        assert_eq!(Key::parse("Right").unwrap(), Key::Right);
        assert_eq!(Key::parse("ArrowRight").unwrap(), Key::Right);
    }

    #[test]
    fn test_parse_navigation_keys() {
        assert_eq!(Key::parse("Home").unwrap(), Key::Home);
        assert_eq!(Key::parse("End").unwrap(), Key::End);
        assert_eq!(Key::parse("PageUp").unwrap(), Key::PageUp);
        assert_eq!(Key::parse("PgUp").unwrap(), Key::PageUp);
        assert_eq!(Key::parse("PageDown").unwrap(), Key::PageDown);
        assert_eq!(Key::parse("PgDown").unwrap(), Key::PageDown);
    }

    #[test]
    fn test_parse_function_keys() {
        assert_eq!(Key::parse("F1").unwrap(), Key::F1);
        assert_eq!(Key::parse("f1").unwrap(), Key::F1);
        assert_eq!(Key::parse("F12").unwrap(), Key::F12);
    }

    #[test]
    fn test_parse_ctrl_combinations() {
        assert_eq!(Key::parse("Ctrl+c").unwrap(), Key::Ctrl('c'));
        assert_eq!(Key::parse("Ctrl+C").unwrap(), Key::Ctrl('C'));
        assert_eq!(Key::parse("ctrl+a").unwrap(), Key::Ctrl('a'));
    }

    #[test]
    fn test_parse_alt_combinations() {
        assert_eq!(Key::parse("Alt+x").unwrap(), Key::Alt('x'));
        assert_eq!(Key::parse("alt+X").unwrap(), Key::Alt('X'));
    }

    #[test]
    fn test_parse_single_char() {
        assert_eq!(Key::parse("a").unwrap(), Key::Char('a'));
        assert_eq!(Key::parse("A").unwrap(), Key::Char('A'));
        assert_eq!(Key::parse("1").unwrap(), Key::Char('1'));
    }

    #[test]
    fn test_parse_invalid() {
        assert!(Key::parse("InvalidKey").is_err());
        assert!(Key::parse("Ctrl+ab").is_err());
        assert!(Key::parse("").is_err());
    }

    #[test]
    fn test_escape_sequences_special() {
        assert_eq!(Key::Enter.to_escape_sequence(), vec![b'\r']);
        assert_eq!(Key::Tab.to_escape_sequence(), vec![b'\t']);
        assert_eq!(Key::Escape.to_escape_sequence(), vec![0x1b]);
        assert_eq!(Key::Backspace.to_escape_sequence(), vec![0x7f]);
        assert_eq!(Key::Space.to_escape_sequence(), vec![b' ']);
    }

    #[test]
    fn test_escape_sequences_arrows() {
        assert_eq!(Key::Up.to_escape_sequence(), b"\x1b[A".to_vec());
        assert_eq!(Key::Down.to_escape_sequence(), b"\x1b[B".to_vec());
        assert_eq!(Key::Right.to_escape_sequence(), b"\x1b[C".to_vec());
        assert_eq!(Key::Left.to_escape_sequence(), b"\x1b[D".to_vec());
    }

    #[test]
    fn test_escape_sequences_navigation() {
        assert_eq!(Key::Home.to_escape_sequence(), b"\x1b[H".to_vec());
        assert_eq!(Key::End.to_escape_sequence(), b"\x1b[F".to_vec());
        assert_eq!(Key::PageUp.to_escape_sequence(), b"\x1b[5~".to_vec());
        assert_eq!(Key::PageDown.to_escape_sequence(), b"\x1b[6~".to_vec());
        assert_eq!(Key::Delete.to_escape_sequence(), b"\x1b[3~".to_vec());
        assert_eq!(Key::Insert.to_escape_sequence(), b"\x1b[2~".to_vec());
    }

    #[test]
    fn test_escape_sequences_function_keys() {
        assert_eq!(Key::F1.to_escape_sequence(), b"\x1bOP".to_vec());
        assert_eq!(Key::F2.to_escape_sequence(), b"\x1bOQ".to_vec());
        assert_eq!(Key::F3.to_escape_sequence(), b"\x1bOR".to_vec());
        assert_eq!(Key::F4.to_escape_sequence(), b"\x1bOS".to_vec());
        assert_eq!(Key::F5.to_escape_sequence(), b"\x1b[15~".to_vec());
        assert_eq!(Key::F12.to_escape_sequence(), b"\x1b[24~".to_vec());
    }

    #[test]
    fn test_escape_sequences_ctrl() {
        // Ctrl+A = 0x01, Ctrl+C = 0x03, Ctrl+Z = 0x1A
        assert_eq!(Key::Ctrl('a').to_escape_sequence(), vec![0x01]);
        assert_eq!(Key::Ctrl('A').to_escape_sequence(), vec![0x01]);
        assert_eq!(Key::Ctrl('c').to_escape_sequence(), vec![0x03]);
        assert_eq!(Key::Ctrl('C').to_escape_sequence(), vec![0x03]);
        assert_eq!(Key::Ctrl('z').to_escape_sequence(), vec![0x1a]);
    }

    #[test]
    fn test_escape_sequences_alt() {
        // Alt+x = ESC x
        assert_eq!(Key::Alt('x').to_escape_sequence(), vec![0x1b, b'x']);
        assert_eq!(Key::Alt('A').to_escape_sequence(), vec![0x1b, b'A']);
    }

    #[test]
    fn test_escape_sequences_char() {
        assert_eq!(Key::Char('a').to_escape_sequence(), vec![b'a']);
        assert_eq!(Key::Char('Z').to_escape_sequence(), vec![b'Z']);
        assert_eq!(Key::Char('1').to_escape_sequence(), vec![b'1']);
    }

    #[test]
    fn test_escape_sequences_unicode_char() {
        // Unicode characters should be encoded as UTF-8
        let key = Key::Char('\u{03B1}'); // Greek letter alpha
        let seq = key.to_escape_sequence();
        assert_eq!(seq, "\u{03B1}".as_bytes().to_vec());
    }
}
