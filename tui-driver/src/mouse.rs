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

/// Generates a mouse move/hover event in SGR 1006 format.
///
/// In SGR 1006 mode, a mouse move event is represented by button code 35
/// (32 for move + 3 for no button) with the 'M' suffix.
///
/// # Arguments
///
/// * `x` - The 1-based column coordinate
/// * `y` - The 1-based row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequence for the mouse move event.
///
/// # Example
///
/// ```
/// use tui_driver::mouse::mouse_move;
///
/// // Generate a hover event at column 10, row 5
/// let hover = mouse_move(10, 5);
/// assert_eq!(hover, b"\x1b[<35;10;5M".to_vec());
/// ```
pub fn mouse_move(x: u16, y: u16) -> Vec<u8> {
    // Button code 35 = 32 (motion) + 3 (no button pressed)
    format!("\x1b[<35;{};{}M", x, y).into_bytes()
}

/// Generates a mouse drag event (press, move, release) in SGR 1006 format.
///
/// This simulates dragging from (start_x, start_y) to (end_x, end_y) using
/// a simple two-point drag: press at start, move to end, release at end.
///
/// # Arguments
///
/// * `button` - The mouse button to use for dragging
/// * `start_x` - The 1-based starting column coordinate
/// * `start_y` - The 1-based starting row coordinate
/// * `end_x` - The 1-based ending column coordinate
/// * `end_y` - The 1-based ending row coordinate
///
/// # Returns
///
/// A vector of bytes representing the escape sequences for a drag operation.
///
/// # Example
///
/// ```
/// use tui_driver::mouse::{mouse_drag, MouseButton};
///
/// // Generate a left-button drag from (1,1) to (10,5)
/// let drag = mouse_drag(MouseButton::Left, 1, 1, 10, 5);
/// // Press at (1,1), motion to (10,5), release at (10,5)
/// ```
pub fn mouse_drag(button: MouseButton, start_x: u16, start_y: u16, end_x: u16, end_y: u16) -> Vec<u8> {
    let mut result = mouse_press(button, start_x, start_y);
    // Motion with button held = 32 + button code
    let motion_code = 32 + button.button_code();
    result.extend(format!("\x1b[<{};{};{}M", motion_code, end_x, end_y).into_bytes());
    result.extend(mouse_release(button, end_x, end_y));
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

    #[test]
    fn test_mouse_move() {
        let seq = mouse_move(10, 5);
        assert_eq!(seq, b"\x1b[<35;10;5M".to_vec());
    }

    #[test]
    fn test_mouse_move_large_coords() {
        let seq = mouse_move(500, 300);
        assert_eq!(seq, b"\x1b[<35;500;300M".to_vec());
    }

    #[test]
    fn test_mouse_drag_left() {
        let seq = mouse_drag(MouseButton::Left, 1, 1, 10, 5);
        // Press at (1,1), motion with button 0 held (code 32), release at (10,5)
        assert_eq!(seq, b"\x1b[<0;1;1M\x1b[<32;10;5M\x1b[<0;10;5m".to_vec());
    }

    #[test]
    fn test_mouse_drag_right() {
        let seq = mouse_drag(MouseButton::Right, 5, 5, 20, 20);
        // Press at (5,5), motion with button 2 held (code 34), release at (20,20)
        assert_eq!(seq, b"\x1b[<2;5;5M\x1b[<34;20;20M\x1b[<2;20;20m".to_vec());
    }
}
