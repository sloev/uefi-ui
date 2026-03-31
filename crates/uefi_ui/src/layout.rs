//! Layout helpers: split [`Rectangle`]s into **rows**, **columns**, and **grids** (panels).
//!
//! All sizes are in pixels; gaps are applied between cells.

use alloc::vec::Vec;

use embedded_graphics::geometry::Point;
use embedded_graphics::prelude::Size;
use embedded_graphics::primitives::Rectangle;

/// Left-aligned horizontal row: each cell has width `widths[i]` (content-sized), `gap` px between
/// cells. Does **not** stretch to fill `area` (flex-start / fit-content behavior).
pub fn row_panels_fit_start(area: Rectangle, widths: &[u32], gap: u32) -> Vec<Rectangle> {
    if widths.is_empty() {
        return Vec::new();
    }
    let ah = area.size.height.max(1) as i32;
    let g = gap as i32;
    let mut x = area.top_left.x;
    let y = area.top_left.y;
    let mut out = Vec::with_capacity(widths.len());
    for (i, &cw) in widths.iter().enumerate() {
        let w = (cw.max(1)) as i32;
        out.push(Rectangle::new(
            Point::new(x, y),
            Size::new(w as u32, ah.max(1) as u32),
        ));
        x += w;
        if i + 1 < widths.len() {
            x += g;
        }
    }
    out
}

/// Horizontal stack: `count` rectangles of equal width inside `area`, with `gap` px between.
pub fn row_panels(area: Rectangle, count: usize, gap: u32) -> Vec<Rectangle> {
    if count == 0 {
        return Vec::new();
    }
    let aw = area.size.width as i32;
    let ah = area.size.height.max(1) as i32;
    let g = gap as i32;
    let inner = aw - g * (count as i32 - 1).max(0);
    let base = inner / count as i32;
    let rem = inner % count as i32;
    let mut x = area.top_left.x;
    let y = area.top_left.y;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let extra = if (i as i32) < rem { 1 } else { 0 };
        let w = base + extra;
        out.push(Rectangle::new(
            Point::new(x, y),
            Size::new(w.max(1) as u32, ah.max(1) as u32),
        ));
        x += w + g;
    }
    out
}

/// Vertical stack: `count` rectangles of equal height inside `area`, with `gap` px between.
pub fn column_panels(area: Rectangle, count: usize, gap: u32) -> Vec<Rectangle> {
    if count == 0 {
        return Vec::new();
    }
    let aw = area.size.width.max(1) as i32;
    let ah = area.size.height as i32;
    let g = gap as i32;
    let inner = ah - g * (count as i32 - 1).max(0);
    let base = inner / count as i32;
    let rem = inner % count as i32;
    let x = area.top_left.x;
    let mut y = area.top_left.y;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let extra = if (i as i32) < rem { 1 } else { 0 };
        let h = base + extra;
        out.push(Rectangle::new(
            Point::new(x, y),
            Size::new(aw.max(1) as u32, h.max(1) as u32),
        ));
        y += h + g;
    }
    out
}

/// `cols × rows` grid in reading order, uniform cells, `gap` px gutters.
pub fn grid_panels(area: Rectangle, cols: usize, rows: usize, gap: u32) -> Vec<Rectangle> {
    if cols == 0 || rows == 0 {
        return Vec::new();
    }
    let aw = area.size.width as i32;
    let ah = area.size.height as i32;
    let gx = gap as i32;
    let gy = gap as i32;
    let inner_w = aw - gx * (cols as i32 - 1).max(0);
    let inner_h = ah - gy * (rows as i32 - 1).max(0);
    let cw = (inner_w / cols as i32).max(1);
    let ch = (inner_h / rows as i32).max(1);
    let mut out = Vec::with_capacity(cols * rows);
    let mut y = area.top_left.y;
    for _r in 0..rows {
        let mut x = area.top_left.x;
        for _c in 0..cols {
            out.push(Rectangle::new(
                Point::new(x, y),
                Size::new(cw as u32, ch as u32),
            ));
            x += cw + gx;
        }
        y += ch + gy;
    }
    out
}

/// Inset `area` by uniform padding (min size 1×1).
pub fn pad(area: Rectangle, pad_px: u32) -> Rectangle {
    let p = pad_px as i32;
    let w = (area.size.width as i32 - 2 * p).max(1);
    let h = (area.size.height as i32 - 2 * p).max(1);
    Rectangle::new(
        Point::new(area.top_left.x + p, area.top_left.y + p),
        Size::new(w as u32, h as u32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::Point;

    #[test]
    fn row_fit_start_left_packs() {
        let r = Rectangle::new(Point::zero(), Size::new(500, 20));
        let v = row_panels_fit_start(r, &[30, 40, 22], 4);
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].top_left, Point::zero());
        assert_eq!(v[0].size.width, 30);
        assert_eq!(v[1].top_left.x, 34);
        assert_eq!(v[2].top_left.x, 78);
    }

    #[test]
    fn column_three() {
        let r = Rectangle::new(Point::zero(), Size::new(100, 100));
        let v = column_panels(r, 3, 4);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn grid_2x2() {
        let r = Rectangle::new(Point::zero(), Size::new(100, 100));
        let g = grid_panels(r, 2, 2, 4);
        assert_eq!(g.len(), 4);
    }
}
