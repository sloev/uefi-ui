//! Keyboard-driven multi-window focus and per-window placement nudging.

/// How many top-level windows participate in **F6 / F7** (or app-defined) focus cycling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowStack {
    pub count: usize,
    pub focused: usize,
}

impl WindowStack {
    pub fn new(count: usize) -> Self {
        let count = count.max(1);
        Self { count, focused: 0 }
    }

    pub fn focus_next(&mut self) {
        self.focused = (self.focused + 1) % self.count;
    }

    pub fn focus_prev(&mut self) {
        self.focused = (self.focused + self.count - 1) % self.count;
    }
}

/// Pixel offset of a window’s top-left from its default [`crate::layout`] anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowOffset {
    pub x: i32,
    pub y: i32,
}

impl WindowOffset {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn nudge(&mut self, dx: i32, dy: i32, min_x: i32, min_y: i32, max_x: i32, max_y: i32) {
        self.x = (self.x + dx).clamp(min_x, max_x);
        self.y = (self.y + dy).clamp(min_y, max_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_wraps() {
        let mut s = WindowStack::new(3);
        assert_eq!(s.focused, 0);
        s.focus_next();
        assert_eq!(s.focused, 1);
        s.focus_prev();
        assert_eq!(s.focused, 0);
        s.focus_prev();
        assert_eq!(s.focused, 2);
    }
}
