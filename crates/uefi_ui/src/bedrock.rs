//! **Bedrock UI chrome**: classic 3D bevels.
//!
//! Pair with [`Theme::bedrock_classic`](crate::theme::Theme::bedrock_classic). For custom chrome, copy the
//! patterns here or see `docs/THEMING.md`.

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};

/// Classic Bedrock 3D palette — exact Bedrock v4 values.
///
/// Four distinct edge colors match the CSS `createBorderStyles` system:
/// - `border_lightest` (#fefefe) — outer top-left
/// - `border_light`   (#dfdfdf) — inner top-left
/// - `border_dark`    (#848584) — inner bottom-right
/// - `border_darkest` (#0a0a0a) — outer bottom-right
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BedrockBevel {
    /// Button/panel face — `material` (#c6c6c6).
    pub face: Rgb888,
    /// Outer top-left border — `borderLightest` (#fefefe).
    pub border_lightest: Rgb888,
    /// Inner top-left bevel — `borderLight` (#dfdfdf).
    pub border_light: Rgb888,
    /// Inner bottom-right bevel — `borderDark` (#848584).
    pub border_dark: Rgb888,
    /// Outer bottom-right border — `borderDarkest` (#0a0a0a).
    pub border_darkest: Rgb888,
}

impl BedrockBevel {
    /// Pixel depth of the full 3-layer border (2px outer + 1px inner).
    /// Use `pad(rect, Self::BEVEL_PX)` to get the content area inside any bevel.
    pub const BEVEL_PX: u32 = 3;

    /// Exact Bedrock default theme colors.
    pub const CLASSIC: Self = Self {
        face:            Rgb888::new(0xc6, 0xc6, 0xc6),
        border_lightest: Rgb888::new(0xfe, 0xfe, 0xfe),
        border_light:    Rgb888::new(0xdf, 0xdf, 0xdf),
        border_dark:     Rgb888::new(0x84, 0x85, 0x84),
        border_darkest:  Rgb888::new(0x0a, 0x0a, 0x0a),
    };

    /// **Raised button** (`button` style): 2px outer + 1px inner bevel.
    ///
    /// ```text
    /// outer TL 2px : border_lightest  outer BR 2px : border_darkest
    /// inner TL 1px : border_light     inner BR 1px : border_dark
    /// ```
    pub fn draw_raised<D: DrawTarget<Color = Rgb888>>(
        &self,
        target: &mut D,
        outer: Rectangle,
    ) -> Result<(), D::Error> {
        let x = outer.top_left.x;
        let y = outer.top_left.y;
        let w = outer.size.width as i32;
        let h = outer.size.height as i32;
        if w < 6 || h < 6 {
            return Ok(());
        }
        // Fill face
        Rectangle::new(outer.top_left, outer.size)
            .into_styled(PrimitiveStyle::with_fill(self.face))
            .draw(target)?;

        // Outer 2px border
        // Top 2 rows (full width, TL color — extends through TR corner)
        hline(target, x, x + w - 1, y, self.border_lightest)?;
        hline(target, x, x + w - 1, y + 1, self.border_lightest)?;
        // Left 2 cols (skip top 2px already set)
        vline(target, x, y + 2, y + h - 3, self.border_lightest)?;
        vline(target, x + 1, y + 2, y + h - 3, self.border_lightest)?;
        // Bottom 2 rows (full width, BR color — extends through BL corner)
        hline(target, x, x + w - 1, y + h - 1, self.border_darkest)?;
        hline(target, x, x + w - 1, y + h - 2, self.border_darkest)?;
        // Right 2 cols (skip top 2 and bottom 2 already set)
        vline(target, x + w - 1, y + 2, y + h - 3, self.border_darkest)?;
        vline(target, x + w - 2, y + 2, y + h - 3, self.border_darkest)?;

        // Inner bevel (1px, at offset 2 from outer edge)
        hline(target, x + 2, x + w - 3, y + 2, self.border_light)?;
        vline(target, x + 2, y + 3, y + h - 4, self.border_light)?;
        hline(target, x + 2, x + w - 3, y + h - 3, self.border_dark)?;
        vline(target, x + w - 3, y + 3, y + h - 4, self.border_dark)?;

        Ok(())
    }

    /// **Window / panel border** (`window` style): 2px outer + 1px inner.
    ///
    /// ```text
    /// outer TL 2px : border_light     outer BR 2px : border_darkest
    /// inner TL 1px : border_lightest  inner BR 1px : border_dark
    /// ```
    pub fn draw_raised_soft<D: DrawTarget<Color = Rgb888>>(
        &self,
        target: &mut D,
        outer: Rectangle,
    ) -> Result<(), D::Error> {
        let x = outer.top_left.x;
        let y = outer.top_left.y;
        let w = outer.size.width as i32;
        let h = outer.size.height as i32;
        if w < 6 || h < 6 {
            return Ok(());
        }
        Rectangle::new(outer.top_left, outer.size)
            .into_styled(PrimitiveStyle::with_fill(self.face))
            .draw(target)?;

        // Outer 2px border
        hline(target, x, x + w - 1, y, self.border_light)?;
        hline(target, x, x + w - 1, y + 1, self.border_light)?;
        vline(target, x, y + 2, y + h - 3, self.border_light)?;
        vline(target, x + 1, y + 2, y + h - 3, self.border_light)?;
        hline(target, x, x + w - 1, y + h - 1, self.border_darkest)?;
        hline(target, x, x + w - 1, y + h - 2, self.border_darkest)?;
        vline(target, x + w - 1, y + 2, y + h - 3, self.border_darkest)?;
        vline(target, x + w - 2, y + 2, y + h - 3, self.border_darkest)?;

        // Inner bevel
        hline(target, x + 2, x + w - 3, y + 2, self.border_lightest)?;
        vline(target, x + 2, y + 3, y + h - 4, self.border_lightest)?;
        hline(target, x + 2, x + w - 3, y + h - 3, self.border_dark)?;
        vline(target, x + w - 3, y + 3, y + h - 4, self.border_dark)?;

        Ok(())
    }

    /// **Sunken field** (`field` style): 2px outer + 1px inner, inverted.
    ///
    /// ```text
    /// outer TL 2px : border_dark      outer BR 2px : border_lightest
    /// inner TL 1px : border_darkest   inner BR 1px : border_light
    /// ```
    pub fn draw_sunken<D: DrawTarget<Color = Rgb888>>(
        &self,
        target: &mut D,
        outer: Rectangle,
    ) -> Result<(), D::Error> {
        let x = outer.top_left.x;
        let y = outer.top_left.y;
        let w = outer.size.width as i32;
        let h = outer.size.height as i32;
        if w < 6 || h < 6 {
            return Ok(());
        }
        Rectangle::new(outer.top_left, outer.size)
            .into_styled(PrimitiveStyle::with_fill(self.face))
            .draw(target)?;

        // Outer 2px border
        hline(target, x, x + w - 1, y, self.border_dark)?;
        hline(target, x, x + w - 1, y + 1, self.border_dark)?;
        vline(target, x, y + 2, y + h - 3, self.border_dark)?;
        vline(target, x + 1, y + 2, y + h - 3, self.border_dark)?;
        hline(target, x, x + w - 1, y + h - 1, self.border_lightest)?;
        hline(target, x, x + w - 1, y + h - 2, self.border_lightest)?;
        vline(target, x + w - 1, y + 2, y + h - 3, self.border_lightest)?;
        vline(target, x + w - 2, y + 2, y + h - 3, self.border_lightest)?;

        // Inner bevel
        hline(target, x + 2, x + w - 3, y + 2, self.border_darkest)?;
        vline(target, x + 2, y + 3, y + h - 4, self.border_darkest)?;
        hline(target, x + 2, x + w - 3, y + h - 3, self.border_light)?;
        vline(target, x + w - 3, y + 3, y + h - 4, self.border_light)?;

        Ok(())
    }

    /// **GroupBox / etched border** (`grouping` style): creates a recessed frame.
    ///
    /// ```text
    /// outer TL: border_dark     outer BR: border_lightest
    /// inner TL: border_lightest inner BR: border_dark
    /// ```
    /// Pass `label_gap` as `Some((x_start, pixel_width))` to blank the top border for a label.
    /// Draw a groupbox etched border.
    ///
    /// `label_gap` — `Some((x_start, width))` skips that horizontal strip of the top border so a
    /// label can be placed there. Pass `face_color` to fill the gap background so the border
    /// line does not bleed through the label text.
    pub fn draw_groupbox<D: DrawTarget<Color = Rgb888>>(
        &self,
        target: &mut D,
        outer: Rectangle,
        label_gap: Option<(i32, i32)>,
        face_color: Option<Rgb888>,
    ) -> Result<(), D::Error> {
        let x = outer.top_left.x;
        let y = outer.top_left.y;
        let w = outer.size.width as i32;
        let h = outer.size.height as i32;
        if w < 4 || h < 4 {
            return Ok(());
        }
        // Fill label gap background so border doesn't bleed through text
        if let (Some((gap_start, gap_w)), Some(face)) = (label_gap, face_color) {
            if gap_w > 0 {
                use embedded_graphics::primitives::{PrimitiveStyle, Rectangle as Rect};
                Rect::new(
                    embedded_graphics::prelude::Point::new(gap_start, y),
                    embedded_graphics::prelude::Size::new(gap_w as u32, 2),
                )
                .into_styled(PrimitiveStyle::with_fill(face))
                .draw(target)?;
            }
        }
        // Left col
        vline(target, x, y, y + h - 1, self.border_dark)?;
        vline(target, x + 1, y, y + h - 1, self.border_lightest)?;
        // Right col
        vline(target, x + w - 2, y, y + h - 1, self.border_dark)?;
        vline(target, x + w - 1, y, y + h - 1, self.border_lightest)?;
        // Bottom rows
        hline(target, x, x + w - 1, y + h - 2, self.border_dark)?;
        hline(target, x, x + w - 1, y + h - 1, self.border_lightest)?;
        // Top rows — with optional label gap
        let (gap_start, gap_w) = label_gap.unwrap_or((0, 0));
        for row_y in [y, y + 1] {
            let c = if row_y == y { self.border_dark } else { self.border_lightest };
            if gap_start > x {
                hline(target, x, gap_start - 1, row_y, c)?;
            }
            let gap_end = gap_start + gap_w;
            if gap_end < x + w {
                hline(target, gap_end, x + w - 1, row_y, c)?;
            }
        }
        Ok(())
    }

    /// **Status-bar border** (`status` style): single-layer, 1px TL dark / 1px BR light.
    pub fn draw_status_border<D: DrawTarget<Color = Rgb888>>(
        &self,
        target: &mut D,
        outer: Rectangle,
    ) -> Result<(), D::Error> {
        let x = outer.top_left.x;
        let y = outer.top_left.y;
        let w = outer.size.width as i32;
        let h = outer.size.height as i32;
        if w < 2 || h < 2 {
            return Ok(());
        }
        hline(target, x, x + w - 1, y, self.border_dark)?;
        vline(target, x, y, y + h - 1, self.border_dark)?;
        hline(target, x, x + w - 1, y + h - 1, self.border_lightest)?;
        vline(target, x + w - 1, y, y + h - 1, self.border_lightest)?;
        Ok(())
    }
}

pub fn hline<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x0: i32,
    x1: i32,
    y: i32,
    c: Rgb888,
) -> Result<(), D::Error> {
    if x0 > x1 {
        return Ok(());
    }
    Line::new(Point::new(x0, y), Point::new(x1, y))
        .into_styled(PrimitiveStyle::with_stroke(c, 1))
        .draw(target)
}

pub fn vline<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y0: i32,
    y1: i32,
    c: Rgb888,
) -> Result<(), D::Error> {
    if y0 > y1 {
        return Ok(());
    }
    Line::new(Point::new(x, y0), Point::new(x, y1))
        .into_styled(PrimitiveStyle::with_stroke(c, 1))
        .draw(target)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::framebuffer::BgrxFramebuffer;

    #[test]
    fn bedrock_bevel_runs() {
        let mut buf = vec![0u8; 200 * 200 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 200, 200, 200 * 4).expect("fb");
        let r = Rectangle::new(Point::new(10, 10), Size::new(80, 32));
        BedrockBevel::CLASSIC.draw_raised(&mut fb, r).unwrap();
        BedrockBevel::CLASSIC.draw_sunken(&mut fb, r).unwrap();
        BedrockBevel::CLASSIC.draw_raised_soft(&mut fb, r).unwrap();
    }

    #[test]
    fn groupbox_runs() {
        let mut buf = vec![0u8; 200 * 200 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 200, 200, 200 * 4).expect("fb");
        let r = Rectangle::new(Point::new(10, 10), Size::new(120, 60));
        BedrockBevel::CLASSIC.draw_groupbox(&mut fb, r, Some((20, 40)), None).unwrap();
    }
}
