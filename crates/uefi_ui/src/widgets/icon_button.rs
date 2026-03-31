//! Icon-only button.

use crate::widgets::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconButton {
    pub pressed: bool,
    pub icon: Icon,
}

impl IconButton {
    pub fn new(icon: Icon) -> Self {
        Self {
            pressed: false,
            icon,
        }
    }

    pub fn set_pressed(&mut self, down: bool) {
        self.pressed = down;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_button_new_and_press() {
        let mut b = IconButton::new(Icon::new('x', 16));
        assert_eq!(b.icon.codepoint, 'x');
        assert_eq!(b.icon.size_px, 16);
        assert!(!b.pressed);
        b.set_pressed(true);
        assert!(b.pressed);
    }
}
