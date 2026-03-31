//! Pointer / mouse state and hit testing (maps from UEFI Simple Pointer later).

use embedded_graphics::geometry::Point;
use embedded_graphics::primitives::Rectangle;

/// Normalized pointer state for one poll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerState {
    pub x: i32,
    pub y: i32,
    pub left: bool,
    pub right: bool,
}

impl PointerState {
    pub const fn new(x: i32, y: i32, left: bool) -> Self {
        Self { x, y, left, right: false }
    }

    pub const fn with_buttons(x: i32, y: i32, left: bool, right: bool) -> Self {
        Self { x, y, left, right }
    }
}

/// Hit-test which horizontal strip item index is under `(x, y)`, or `None`.
pub fn index_at(items: &[Rectangle], state: &PointerState) -> Option<usize> {
    let p = Point::new(state.x, state.y);
    items.iter().position(|r| r.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::Size;

    #[test]
    fn s5_index_at_finds_rect() {
        let items = [
            Rectangle::new(Point::new(0, 0), Size::new(50, 20)),
            Rectangle::new(Point::new(50, 0), Size::new(50, 20)),
        ];
        let p = PointerState::new(60, 10, true);
        assert_eq!(index_at(&items, &p), Some(1));
    }
}
