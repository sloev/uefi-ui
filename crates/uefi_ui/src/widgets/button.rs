//! Push button interaction state.

/// Immediate-mode button: `pressed` tracks pointer-down inside the hit rect.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Button {
    pub pressed: bool,
}

impl Button {
    pub const fn new() -> Self {
        Self { pressed: false }
    }

    pub fn set_pressed(&mut self, down: bool) {
        self.pressed = down;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_default_not_pressed() {
        assert!(!Button::default().pressed);
        assert!(!Button::new().pressed);
    }

    #[test]
    fn button_set_pressed() {
        let mut b = Button::new();
        b.set_pressed(true);
        assert!(b.pressed);
        b.set_pressed(false);
        assert!(!b.pressed);
    }
}
