//! BGRX CPU framebuffer implementing [`DrawTarget`] for UEFI GOP-style buffers.
//!
//! # Performance on UEFI
//!
//! - **Solid rectangles:** Prefer [`BgrxFramebuffer::fill_rect_solid`] over drawing a filled
//!   [`embedded_graphics::primitives::Rectangle`] through [`DrawTarget`]. The generic path
//!   invokes one `draw_iter` step per pixel; bulk fill uses tight row loops (and lets LLVM
//!   vectorize or `memset`-style the inner copy).
//! - **GOP `Blt` vs CPU:** For large uniform fills, `uefi`’s `BltOp::VideoFill` is often fastest
//!   (firmware may use a GPU fill or DMA). For mixed UI, draw into a RAM buffer with
//!   [`BgrxFramebuffer`], then map the GOP buffer with `GraphicsOutput::frame_buffer` (unless the
//!   mode is `BltOnly`) or upload with `BltOp::BufferToVideo`. Profile on target hardware: many
//!   small blits can be slower than one full-buffer upload.
//! - **Resolution:** Fewer pixels means faster frames — pick a lower GOP mode when available
//!   (`GraphicsOutput::set_mode`).
//! - **Allocations:** Keep fonts and scratch buffers outside the per-frame hot path; reuse
//! [`alloc::vec::Vec`] capacity where rasterizing text or decoding PNGs.

use core::convert::Infallible;

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;

/// BGRX 32-bit framebuffer: each pixel is blue, green, red, unused (`stride` may exceed `width * 4`).
pub struct BgrxFramebuffer<'a> {
    data: &'a mut [u8],
    width: u32,
    height: u32,
    stride_bytes: usize,
}

impl<'a> BgrxFramebuffer<'a> {
    pub fn new(data: &'a mut [u8], width: u32, height: u32, stride_bytes: usize) -> Option<Self> {
        let min = stride_bytes.checked_mul(height as usize)?;
        if data.len() < min || stride_bytes < width as usize * 4 {
            return None;
        }
        Some(Self {
            data,
            width,
            height,
            stride_bytes,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn stride_bytes(&self) -> usize {
        self.stride_bytes
    }

    /// Read back pixel at (x, y) as [`Rgb888`], or `None` if out of range.
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<Rgb888> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let offset = y as usize * self.stride_bytes + x as usize * 4;
        let s = self.data.get(offset..offset + 4)?;
        Some(Rgb888::new(s[2], s[1], s[0]))
    }

    /// Fill a solid rectangle with `c`, clipping to the buffer. Prefer this over a filled
    /// [`embedded_graphics::primitives::Rectangle`] on [`DrawTarget`] when you only need a flat
    /// color (see module docs — **Performance on UEFI**).
    pub fn fill_rect_solid(&mut self, x: u32, y: u32, width: u32, height: u32, c: Rgb888) {
        if width == 0 || height == 0 {
            return;
        }
        let x1 = x.saturating_add(width).min(self.width);
        let y1 = y.saturating_add(height).min(self.height);
        let x0 = x.min(self.width);
        let y0 = y.min(self.height);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let row_px = x1 - x0;
        let px = [c.b(), c.g(), c.r(), 0_u8];
        for row in y0..y1 {
            let start = row as usize * self.stride_bytes + x0 as usize * 4;
            let len = row_px as usize * 4;
            let slice = &mut self.data[start..start + len];
            for chunk in slice.chunks_exact_mut(4) {
                chunk.copy_from_slice(&px);
            }
        }
    }

    #[inline]
    fn write_rgb(&mut self, x: u32, y: u32, c: Rgb888) {
        if x >= self.width || y >= self.height {
            return;
        }
        let offset = y as usize * self.stride_bytes + x as usize * 4;
        if let Some(px) = self.data.get_mut(offset..offset + 4) {
            px[0] = c.b();
            px[1] = c.g();
            px[2] = c.r();
            px[3] = 0;
        }
    }

    /// Alpha-blend `fg` onto the existing pixel (for TrueType glyph masks, `alpha` 0–255).
    pub fn blend_pixel(&mut self, x: u32, y: u32, fg: Rgb888, alpha: u8) {
        if alpha == 0 || x >= self.width || y >= self.height {
            return;
        }
        if alpha == 255 {
            self.write_rgb(x, y, fg);
            return;
        }
        let Some(bg) = self.pixel_at(x, y) else {
            return;
        };
        let a = alpha as u32;
        let blend = |b: u8, f: u8| -> u8 {
            ((b as u32 * (255 - a) + f as u32 * a + 127) / 255) as u8
        };
        let c = Rgb888::new(
            blend(bg.r(), fg.r()),
            blend(bg.g(), fg.g()),
            blend(bg.b(), fg.b()),
        );
        self.write_rgb(x, y, c);
    }
}

impl Dimensions for BgrxFramebuffer<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(Point::zero(), Size::new(self.width, self.height))
    }
}

impl DrawTarget for BgrxFramebuffer<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            if coord.x < 0 || coord.y < 0 {
                continue;
            }
            let Ok(x) = u32::try_from(coord.x) else {
                continue;
            };
            let Ok(y) = u32::try_from(coord.y) else {
                continue;
            };
            self.write_rgb(x, y, color);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::pixelcolor::RgbColor;
    use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

    #[test]
    fn s1_draw_iter_writes_bgrx_and_pixel_at_reads_rgb888() {
        let mut buf = vec![0u8; 16 * 4 * 2];
        let mut fb = BgrxFramebuffer::new(&mut buf, 16, 2, 16 * 4).expect("fb");
        let rect = Rectangle::new(Point::new(1, 0), Size::new(2, 1))
            .into_styled(PrimitiveStyle::with_fill(Rgb888::new(10, 20, 30)));
        rect.draw(&mut fb).unwrap();
        assert_eq!(fb.pixel_at(1, 0), Some(Rgb888::new(10, 20, 30)));
        assert_eq!(fb.pixel_at(2, 0), Some(Rgb888::new(10, 20, 30)));
    }

    #[test]
    fn s1_out_of_bounds_pixel_discarded() {
        let mut buf = vec![0u8; 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 1, 1, 4).expect("fb");
        fb.draw_iter([Pixel(Point::new(-5, 0), Rgb888::WHITE)])
            .unwrap();
        assert_eq!(fb.pixel_at(0, 0), Some(Rgb888::BLACK));
    }

    #[test]
    fn s1_fill_rect_solid_clips_and_matches_color() {
        let mut buf = vec![0u8; 8 * 4 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 8, 4, 8 * 4).expect("fb");
        let c = Rgb888::new(11, 22, 33);
        fb.fill_rect_solid(2, 1, 3, 2, c);
        assert_eq!(fb.pixel_at(2, 1), Some(c));
        assert_eq!(fb.pixel_at(4, 2), Some(c));
        assert_eq!(fb.pixel_at(1, 1), Some(Rgb888::BLACK));
        fb.fill_rect_solid(0, 0, 100, 100, Rgb888::WHITE);
        assert_eq!(fb.pixel_at(7, 3), Some(Rgb888::WHITE));
    }
}
