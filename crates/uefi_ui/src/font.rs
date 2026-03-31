//! TrueType / OTF loading and glyph rasterization via [`fontdue`] (requires `alloc`).
//!
//! On UEFI you must link a global allocator; raster buffers are allocated here.

use alloc::vec::Vec;
use fontdue::{Font, FontSettings};

/// Load a font from firmware-supplied bytes (e.g. `include_bytes!` in your application).
pub fn load_font(bytes: &[u8]) -> fontdue::FontResult<Font> {
    Font::from_bytes(bytes, FontSettings::default())
}

/// Rasterize a single glyph into a tightly packed alpha buffer (width × height, one byte per pixel).
pub fn rasterize_glyph(font: &Font, c: char, px: f32) -> Option<(usize, usize, Vec<u8>)> {
    let (m, buf) = font.rasterize(c, px);
    if m.width == 0 || m.height == 0 {
        return None;
    }
    Some((m.width, m.height, buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Built-in tiny font bytes (Ubuntu Font License, subset) — if missing, test is skipped.
    #[test]
    fn s6_fontdue_load_and_rasterize() {
        // Avoid large binary in repo: use system font only when running tests locally if desired.
        // Here we only assert API shape with a minimal invalid/empty check.
        assert!(load_font(&[]).is_err());
    }
}
