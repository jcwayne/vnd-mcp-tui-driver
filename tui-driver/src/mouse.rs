//! Mouse event generation module for SGR 1006 extended mouse mode.
//!
//! This module provides functionality to generate mouse event escape sequences
//! using the SGR 1006 format, which supports coordinates beyond the traditional
//! 223 limit of the X10 protocol.
//!
//! SGR 1006 format:
//! - Press:   `\x1b[<BUTTON;X;YM`
//! - Release: `\x1b[<BUTTON;X;Ym`
//! - Coordinates are 1-based

/// Represents mouse buttons that can be clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button (button code 0)
    Left,
    /// Middle mouse button (button code 1)
    Middle,
    /// Right mouse button (button code 2)
    Right,
}

impl MouseButton {
    /// Returns the SGR button code for this mouse button.
    fn button_code(&self) -> u8 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
        }
    }
}

/// Generates a mouse press event in SGR 1006 format.
///
/// # Arguments
///
/// * `button` - The mouse button to press
/// * `x` - The 1-based column coordinate
/// * `y` - The 1-based row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequence for the mouse press event.
fn mouse_press(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    format!("\x1b[<{};{};{}M", button.button_code(), x, y).into_bytes()
}

/// Generates a mouse release event in SGR 1006 format.
///
/// # Arguments
///
/// * `button` - The mouse button to release
/// * `x` - The 1-based column coordinate
/// * `y` - The 1-based row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequence for the mouse release event.
fn mouse_release(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    format!("\x1b[<{};{};{}m", button.button_code(), x, y).into_bytes()
}

/// Generates a complete mouse click (press followed by release) in SGR 1006 format.
///
/// # Arguments
///
/// * `button` - The mouse button to click
/// * `x` - The 1-based column coordinate
/// * `y` - The 1-based row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequences for a complete mouse click
/// (press event followed by release event).
///
/// # Example
///
/// ```
/// use tui_driver::mouse::{mouse_click, MouseButton};
///
/// // Generate a left click at column 10, row 5
/// let click = mouse_click(MouseButton::Left, 10, 5);
/// assert_eq!(click, b"\x1b[<0;10;5M\x1b[<0;10;5m".to_vec());
/// ```
pub fn mouse_click(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    let mut result = mouse_press(button, x, y);
    result.extend(mouse_release(button, x, y));
    result
}

/// Generates a double click (two rapid clicks) in SGR 1006 format.
///
/// # Arguments
///
/// * `button` - The mouse button to double-click
/// * `x` - The 1-based column coordinate
/// * `y` - The 1-based row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequences for a double click
/// (two complete click sequences in succession).
///
/// # Example
///
/// ```
/// use tui_driver::mouse::{mouse_double_click, MouseButton};
///
/// // Generate a left double-click at column 10, row 5
/// let double_click = mouse_double_click(MouseButton::Left, 10, 5);
/// assert_eq!(double_click, b"\x1b[<0;10;5M\x1b[<0;10;5m\x1b[<0;10;5M\x1b[<0;10;5m".to_vec());
/// ```
pub fn mouse_double_click(button: MouseButton, x: u16, y: u16) -> Vec<u8> {
    let mut result = mouse_click(button, x, y);
    result.extend(mouse_click(button, x, y));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_codes() {
        assert_eq!(MouseButton::Left.button_code(), 0);
        assert_eq!(MouseButton::Middle.button_code(), 1);
        assert_eq!(MouseButton::Right.button_code(), 2);
    }

    #[test]
    fn test_mouse_press_left() {
        let seq = mouse_press(MouseButton::Left, 1, 1);
        assert_eq!(seq, b"\x1b[<0;1;1M".to_vec());
    }

    #[test]
    fn test_mouse_press_middle() {
        let seq = mouse_press(MouseButton::Middle, 5, 10);
        assert_eq!(seq, b"\x1b[<1;5;10M".to_vec());
    }

    #[test]
    fn test_mouse_press_right() {
        let seq = mouse_press(MouseButton::Right, 100, 50);
        assert_eq!(seq, b"\x1b[<2;100;50M".to_vec());
    }

    #[test]
    fn test_mouse_release_left() {
        let seq = mouse_release(MouseButton::Left, 1, 1);
        assert_eq!(seq, b"\x1b[<0;1;1m".to_vec());
    }

    #[test]
    fn test_mouse_release_middle() {
        let seq = mouse_release(MouseButton::Middle, 5, 10);
        assert_eq!(seq, b"\x1b[<1;5;10m".to_vec());
    }

    #[test]
    fn test_mouse_release_right() {
        let seq = mouse_release(MouseButton::Right, 100, 50);
        assert_eq!(seq, b"\x1b[<2;100;50m".to_vec());
    }

    #[test]
    fn test_mouse_click_left() {
        let seq = mouse_click(MouseButton::Left, 10, 5);
        assert_eq!(seq, b"\x1b[<0;10;5M\x1b[<0;10;5m".to_vec());
    }

    #[test]
    fn test_mouse_click_middle() {
        let seq = mouse_click(MouseButton::Middle, 20, 15);
        assert_eq!(seq, b"\x1b[<1;20;15M\x1b[<1;20;15m".to_vec());
    }

    #[test]
    fn test_mouse_click_right() {
        let seq = mouse_click(MouseButton::Right, 1, 1);
        assert_eq!(seq, b"\x1b[<2;1;1M\x1b[<2;1;1m".to_vec());
    }

    #[test]
    fn test_mouse_double_click_left() {
        let seq = mouse_double_click(MouseButton::Left, 10, 5);
        let expected = b"\x1b[<0;10;5M\x1b[<0;10;5m\x1b[<0;10;5M\x1b[<0;10;5m".to_vec();
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_mouse_double_click_right() {
        let seq = mouse_double_click(MouseButton::Right, 30, 20);
        let expected = b"\x1b[<2;30;20M\x1b[<2;30;20m\x1b[<2;30;20M\x1b[<2;30;20m".to_vec();
        assert_eq!(seq, expected);
    }

    #[test]
    fn test_large_coordinates() {
        // SGR 1006 supports coordinates beyond 223
        let seq = mouse_click(MouseButton::Left, 500, 300);
        assert_eq!(seq, b"\x1b[<0;500;300M\x1b[<0;500;300m".to_vec());
    }

    #[test]
    fn test_mouse_button_clone_and_eq() {
        let button1 = MouseButton::Left;
        let button2 = button1;
        assert_eq!(button1, button2);

        let button3 = MouseButton::Right;
        assert_ne!(button1, button3);
    }

    #[test]
    fn test_mouse_button_debug() {
        assert_eq!(format!("{:?}", MouseButton::Left), "Left");
        assert_eq!(format!("{:?}", MouseButton::Middle), "Middle");
        assert_eq!(format!("{:?}", MouseButton::Right), "Right");
    }
}
