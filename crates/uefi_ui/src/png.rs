//! PNG decode via [`minipng`] — **no_std**, **no allocator required** for the decoder itself.
//! The caller supplies the output buffer (stack or heap).

use minipng::{decode_png, decode_png_header, ImageData};

/// Decode a PNG into caller-provided RGBA8 buffer; returns view into that buffer.
pub fn decode_png_to_rgba<'a>(bytes: &[u8], buf: &'a mut [u8]) -> Result<ImageData<'a>, minipng::Error> {
    let mut img = decode_png(bytes, buf)?;
    img.convert_to_rgba8bpc()?;
    Ok(img)
}

/// Width / height / required byte count for RGBA8 before decoding.
pub fn png_dimensions_and_size(bytes: &[u8]) -> Result<(u32, u32, usize), minipng::Error> {
    let h = decode_png_header(bytes)?;
    Ok((h.width(), h.height(), h.required_bytes_rgba8bpc()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid 1×1 RGBA PNG (generated offline).
    const ONE_BY_ONE_RGBA: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
        0x42, 0x60, 0x82,
    ];

    #[test]
    fn s7_decode_tiny_png_rgba() {
        let (w, h, need) = png_dimensions_and_size(ONE_BY_ONE_RGBA).expect("header");
        assert_eq!((w, h), (1, 1));
        let mut buf = alloc::vec![0u8; need];
        let img = decode_png_to_rgba(ONE_BY_ONE_RGBA, &mut buf).expect("decode");
        assert_eq!(img.width(), 1);
        assert_eq!(img.height(), 1);
        assert!(img.pixels().len() >= 4);
    }
}
