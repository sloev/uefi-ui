//! Fixed empty space for flex-like layouts.

use embedded_graphics::prelude::Size;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spacer {
    pub size: Size,
}

impl Spacer {
    pub const fn new(width: u32, height: u32) -> Self {
        Self {
            size: Size::new(width, height),
        }
    }
}
