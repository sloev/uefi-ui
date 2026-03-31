//! Draw a single line of text with [`fontdue`] into a [`BgrxFramebuffer`] (demo-only).

use fontdue::Font;
use uefi_ui::embedded_graphics::pixelcolor::Rgb888;
use uefi_ui::framebuffer::BgrxFramebuffer;

#[inline]
fn f32_to_i32_floor(x: f32) -> i32 {
    let t = x as i32;
    if x < t as f32 {
        t - 1
    } else {
        t
    }
}

/// Draw `text` left-to-right; returns the final x advance in the same coordinate space as `x0`.
///
/// `y_baseline` is the **baseline** in screen space (y increases downward). [`fontdue::Metrics`]
/// stores `ymin` as the **bottom** edge of the bitmap relative to the baseline, not the top; the
/// bitmap’s row `0` is the top-left. Correct top-y matches fontdue’s PositiveYDown layout:
/// `baseline_y - bounds.height - bounds.ymin` (see `fontdue::layout`).
pub fn draw_text_line(
    target: &mut BgrxFramebuffer<'_>,
    font: &Font,
    size_px: f32,
    x0: f32,
    y_baseline: f32,
    text: &str,
    color: Rgb888,
) -> f32 {
    let mut x = x0;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size_px);
        let b = metrics.bounds;
        let ox = f32_to_i32_floor(x + b.xmin);
        let oy = f32_to_i32_floor(y_baseline - b.height - b.ymin);
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let a = bitmap[row * metrics.width + col];
                if a == 0 {
                    continue;
                }
                let px = ox + col as i32;
                let py = oy + row as i32;
                if px >= 0 && py >= 0 {
                    let ux = px as u32;
                    let uy = py as u32;
                    if ux < target.width() && uy < target.height() {
                        target.blend_pixel(ux, uy, color, a);
                    }
                }
            }
        }
        x += metrics.advance_width;
    }
    x
}
