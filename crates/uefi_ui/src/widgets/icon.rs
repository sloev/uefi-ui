//! Icon descriptor (draw with your font / PNG atlas in the app).

/// Logical icon: one Unicode codepoint or private-use slot, plus nominal pixel size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Icon {
    pub codepoint: char,
    pub size_px: u32,
}

impl Icon {
    pub const fn new(codepoint: char, size_px: u32) -> Self {
        Self { codepoint, size_px }
    }
}
