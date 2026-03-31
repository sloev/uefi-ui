//! Popover / modal **dialog** state and anchor positioning.
//!
//! Pair with [`crate::theme::Theme`] colors (`popover_bg`, `overlay`) when drawing.

use embedded_graphics::geometry::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;

/// Identifies an open popover (use your own enum in the app, or raw ids).
pub type PopoverId = u64;

/// Whether a dialog blocks the rest of the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopoverKind {
    /// Click-out / other widgets still receive input (you decide).
    Popover,
    /// Dims background; typically only dialog is interactive until closed.
    Modal,
}

#[derive(Debug, Clone)]
pub struct PopoverSpec {
    pub id: PopoverId,
    pub kind: PopoverKind,
}

/// Stack of open popovers (last = top-most).
#[derive(Debug, Clone, Default)]
pub struct PopoverStack {
    pub open: alloc::vec::Vec<PopoverSpec>,
}

impl PopoverStack {
    pub fn push(&mut self, spec: PopoverSpec) {
        self.open.retain(|s| s.id != spec.id);
        self.open.push(spec);
    }

    pub fn pop(&mut self) -> Option<PopoverSpec> {
        self.open.pop()
    }

    pub fn dismiss(&mut self, id: PopoverId) {
        self.open.retain(|s| s.id != id);
    }

    pub fn clear(&mut self) {
        self.open.clear();
    }

    pub fn top(&self) -> Option<&PopoverSpec> {
        self.open.last()
    }

    pub fn is_modal_blocking(&self) -> bool {
        self.open.last().map(|s| s.kind == PopoverKind::Modal).unwrap_or(false)
    }
}

/// Place a popover rectangle **below** an anchor (e.g. toolbar button), clamped to `screen`.
pub fn place_below_anchor(anchor: Rectangle, content: Size, screen: Rectangle) -> Rectangle {
    let mut top_left = Point::new(
        anchor.top_left.x,
        anchor.top_left.y + anchor.size.height as i32,
    );
    let max_x = screen.top_left.x + screen.size.width as i32 - content.width as i32;
    let max_y = screen.top_left.y + screen.size.height as i32 - content.height as i32;
    top_left.x = top_left.x.clamp(screen.top_left.x, max_x);
    top_left.y = top_left.y.clamp(screen.top_left.y, max_y);
    Rectangle::new(top_left, content)
}

/// Center a rectangle of `content` size inside `screen`.
pub fn center_in_screen(content: Size, screen: Rectangle) -> Rectangle {
    let x = screen.top_left.x + (screen.size.width as i32 - content.width as i32) / 2;
    let y = screen.top_left.y + (screen.size.height as i32 - content.height as i32) / 2;
    Rectangle::new(Point::new(x, y), content)
}
