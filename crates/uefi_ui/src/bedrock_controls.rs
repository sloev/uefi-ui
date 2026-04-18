//! Classic **Bedrock control chrome** (checkbox, radio, slider, combo shell, focus ring).
//!
//! These helpers are **optional drawing utilities**: [`crate::widgets`] stay state-only; any visual
//! style (flat, Bedrock, Mac, custom) is composed in **your** application. This module encodes the
//! 3D bevel look via [`crate::bedrock::BedrockBevel`].

use embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle};
use embedded_graphics::text::{Baseline, Text};

use crate::layout::pad;
use crate::bedrock::{hline, vline, BedrockBevel};

// ── Menubar shared metrics ─────────────────────────────────────────────────────
/// Horizontal padding added on each side of a menu item label.
pub const MENU_ITEM_PAD_X: i32 = 4;
/// Vertical offset of label baseline inside a menu cell.
pub const MENU_ITEM_PAD_Y: i32 = 5;
/// Left padding for items inside a popup dropdown.
pub const MENU_POPUP_PAD_X: i32 = 6;
/// Top inset of first popup row from the popup top edge.
pub const MENU_POPUP_TOP_PAD: i32 = 4;

/// Draw text in the classic embossed disabled style.
///
/// Renders `label` twice: first offset (+1,+1) in `border_lightest` (white shadow), then at the
/// original position in `border_dark` (gray). Produces the "grayed + embossed" disabled text used
/// on all inactive controls.
pub fn draw_label_disabled<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    label: &str,
    font: &MonoFont<'_>,
    x: i32,
    y: i32,
    border_dark: Rgb888,
    border_lightest: Rgb888,
) -> Result<(), D::Error> {
    let shadow_style = MonoTextStyle::new(font, border_lightest);
    Text::with_baseline(label, Point::new(x + 1, y + 1), shadow_style, Baseline::Top).draw(target)?;
    let gray_style = MonoTextStyle::new(font, border_dark);
    Text::with_baseline(label, Point::new(x, y), gray_style, Baseline::Top).draw(target)?;
    Ok(())
}

/// Sunken 20×20 checkbox with optional checkmark (Bedrock size, 4-layer border).
pub fn draw_checkbox_classic<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    outer: Rectangle,
    checked: bool,
    paper: Rgb888,
    enabled: bool,
) -> Result<(), D::Error> {
    bevel.draw_sunken(target, outer)?;
    // Inner area: white when enabled, light gray when disabled
    let fill = if enabled { paper } else { bevel.border_light };
    let inner = pad(outer, BedrockBevel::BEVEL_PX);
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(target)?;
    if checked {
        // Checkmark ink: darkest when enabled, mid-gray when disabled
        let ink = if enabled { bevel.border_darkest } else { bevel.border_dark };
        let cx = outer.top_left.x + outer.size.width as i32 / 2;
        let cy = outer.top_left.y + outer.size.height as i32 / 2;
        Line::new(Point::new(cx - 4, cy + 1), Point::new(cx - 1, cy + 4))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(target)?;
        Line::new(Point::new(cx - 1, cy + 4), Point::new(cx + 5, cy - 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(target)?;
    }
    Ok(())
}

/// Horizontal row of radio buttons — Bedrock size: 20px outer circle, 8px inner dot.
///
/// When `enabled = false`, the interior uses `border_light` instead of white
/// and the selection dot uses `border_dark` instead of `border_darkest`.
pub fn draw_radio_row<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    row: Rectangle,
    count: usize,
    selected: usize,
    bevel: &BedrockBevel,
    enabled: bool,
) -> Result<(), D::Error> {
    let mut x = row.top_left.x;
    let cy = row.top_left.y + row.size.height as i32 / 2;
    let d = 20u32; // outer diameter
    let gap = 8;
    let interior = if enabled { Rgb888::WHITE } else { bevel.border_light };
    let dot_ink = if enabled { bevel.border_darkest } else { bevel.border_dark };
    for i in 0..count {
        let tl = Point::new(x, cy - d as i32 / 2);
        // Layer 1: outermost — border_darkest (black ring)
        Circle::new(tl, d)
            .into_styled(PrimitiveStyle::with_fill(bevel.border_darkest))
            .draw(target)?;
        // Layer 2: border_dark (gray ring, 1px inset)
        Circle::new(tl + Point::new(1, 1), d - 2)
            .into_styled(PrimitiveStyle::with_fill(bevel.border_dark))
            .draw(target)?;
        // Layer 3: interior (2px inset) — white or light gray when disabled
        Circle::new(tl + Point::new(2, 2), d - 4)
            .into_styled(PrimitiveStyle::with_fill(interior))
            .draw(target)?;
        if i == selected {
            // Filled dot: 8×8 px, centered inside the white area
            let dot_d = 8u32;
            let dot_tl = Point::new(
                x + (d as i32 - dot_d as i32) / 2,
                cy - dot_d as i32 / 2,
            );
            Circle::new(dot_tl, dot_d)
                .into_styled(PrimitiveStyle::with_fill(dot_ink))
                .draw(target)?;
        }
        x += d as i32 + gap;
    }
    Ok(())
}

/// `ratio` in 0..=1 along the track; `thumb_w` raised thumb width.
pub fn draw_slider_track_thumb<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    track: Rectangle,
    ratio: f32,
    thumb_w: u32,
) -> Result<(), D::Error> {
    bevel.draw_sunken(target, track)?;
    let inner = pad(track, BedrockBevel::BEVEL_PX);
    let t = ratio.clamp(0.0, 1.0);
    let span = inner.size.width.saturating_sub(thumb_w);
    let tx = inner.top_left.x + (span as f32 * t) as i32;
    let thumb = Rectangle::new(
        Point::new(tx, inner.top_left.y),
        Size::new(thumb_w, inner.size.height),
    );
    bevel.draw_raised(target, thumb)
}

/// Sunken "text field" fill (caller draws text). Inner white at 3px inset.
pub fn draw_sunken_field<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    field: Rectangle,
    paper: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_sunken(target, field)?;
    let inner = pad(field, BedrockBevel::BEVEL_PX);
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(paper))
        .draw(target)
}

/// Push button: raised bevel with centered text label.
///
/// - `pressed`    — inverts bevel and shifts label 1 px right+down.
/// - `is_default` — adds an extra 1 px black outer ring (the button activated by `Enter`).
/// - `enabled`    — when `false`, draws the label in the embossed disabled style (gray + shadow).
pub fn draw_button<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    label: &str,
    font: &MonoFont<'_>,
    ink: Rgb888,
    pressed: bool,
    is_default: bool,
    enabled: bool,
) -> Result<(), D::Error> {
    // Default button outer ring (drawn before bevel so bevel fills over it)
    if is_default {
        rect.into_styled(PrimitiveStyle::with_stroke(bevel.border_darkest, 1))
            .draw(target)?;
        let inner = pad(rect, 1);
        if pressed {
            draw_raised_pressed(target, bevel, inner)?;
        } else {
            bevel.draw_raised(target, inner)?;
        }
    } else if pressed {
        draw_raised_pressed(target, bevel, rect)?;
    } else {
        bevel.draw_raised(target, rect)?;
    }
    // Center label
    let char_w = font.character_size.width as i32;
    let char_h = font.character_size.height as i32;
    let text_w = label.len() as i32 * char_w;
    let offset = if pressed { 1 } else { 0 };
    let tx = rect.top_left.x + (rect.size.width as i32 - text_w) / 2 + offset;
    let ty = rect.top_left.y + (rect.size.height as i32 - char_h) / 2 + offset;
    if enabled {
        let style = MonoTextStyle::new(font, ink);
        Text::with_baseline(label, Point::new(tx, ty), style, Baseline::Top).draw(target)?;
    } else {
        draw_label_disabled(target, label, font, tx, ty, bevel.border_dark, bevel.border_lightest)?;
    }
    Ok(())
}

/// Combo box chrome: sunken value field + raised dropdown button (caller draws label + glyph).
pub fn draw_combobox_chrome<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    field: Rectangle,
    button: Rectangle,
    paper: Rgb888,
) -> Result<(), D::Error> {
    draw_sunken_field(target, bevel, field, paper)?;
    bevel.draw_raised(target, button)
}

/// 1px solid focus ring **outside** the widget bounds (used for whole-widget focus).
pub fn draw_focus_ring<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    widget: Rectangle,
    color: Rgb888,
) -> Result<(), D::Error> {
    let outer = Rectangle::new(
        Point::new(widget.top_left.x - 1, widget.top_left.y - 1),
        Size::new(widget.size.width + 2, widget.size.height + 2),
    );
    outer
        .into_styled(PrimitiveStyle::with_stroke(color, 1))
        .draw(target)
}

/// Dotted 1px focus rectangle **inside** the widget bounds (keyboard cursor inside a list/field).
/// Draws alternating pixels along the perimeter using `(x+y) % 2 == 0`.
pub fn draw_focus_rect<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    rect: Rectangle,
    color: Rgb888,
) -> Result<(), D::Error> {
    let x0 = rect.top_left.x;
    let y0 = rect.top_left.y;
    let x1 = x0 + rect.size.width as i32 - 1;
    let y1 = y0 + rect.size.height as i32 - 1;
    if x1 < x0 || y1 < y0 { return Ok(()); }
    // Top and bottom edges
    for x in x0..=x1 {
        if (x + y0) % 2 == 0 { Pixel(Point::new(x, y0), color).draw(target)?; }
        if (x + y1) % 2 == 0 { Pixel(Point::new(x, y1), color).draw(target)?; }
    }
    // Left and right edges (skip corners already drawn)
    for y in (y0 + 1)..y1 {
        if (x0 + y) % 2 == 0 { Pixel(Point::new(x0, y), color).draw(target)?; }
        if (x1 + y) % 2 == 0 { Pixel(Point::new(x1, y), color).draw(target)?; }
    }
    Ok(())
}

/// Sunken progress track with filled portion `value` in 0..=1.
/// Bedrock: white canvas track, navy progress fill.
pub fn draw_progress_bar<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    track: Rectangle,
    value: f32,
    track_color: Rgb888,
    fill_color: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_sunken(target, track)?;
    let inner = pad(track, BedrockBevel::BEVEL_PX);
    // Fill track background
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(track_color))
        .draw(target)?;
    let v = value.clamp(0.0, 1.0);
    let filled_w = ((inner.size.width as f32) * v) as i32;
    // Draw 8px chunks with 2px gaps
    const CHUNK: i32 = 8;
    const GAP: i32 = 2;
    let mut cx = inner.top_left.x;
    while cx + CHUNK <= inner.top_left.x + filled_w {
        Rectangle::new(
            Point::new(cx, inner.top_left.y),
            Size::new(CHUNK as u32, inner.size.height),
        )
        .into_styled(PrimitiveStyle::with_fill(fill_color))
        .draw(target)?;
        cx += CHUNK + GAP;
    }
    // Partial chunk at the end
    let remaining = (inner.top_left.x + filled_w) - cx;
    if remaining > 0 {
        Rectangle::new(
            Point::new(cx, inner.top_left.y),
            Size::new(remaining as u32, inner.size.height),
        )
        .into_styled(PrimitiveStyle::with_fill(fill_color))
        .draw(target)?;
    }
    Ok(())
}

/// One row inside a list box.
/// - `selected` — fills with `selection` color (navy + white text).
/// - `focused`  — when focused but not selected, draws a dotted inner focus rect.
pub fn draw_listbox_row<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    list_inner: Rectangle,
    row_y: i32,
    row_h: i32,
    selected: bool,
    focused: bool,
    paper: Rgb888,
    selection: Rgb888,
) -> Result<(), D::Error> {
    let row = Rectangle::new(
        Point::new(list_inner.top_left.x + 1, row_y),
        Size::new(list_inner.size.width.saturating_sub(2), row_h as u32),
    );
    let bg = if selected { selection } else { paper };
    Rectangle::new(row.top_left, row.size)
        .into_styled(PrimitiveStyle::with_fill(bg))
        .draw(target)?;
    if focused && !selected {
        draw_focus_rect(target, row, Rgb888::new(0x0a, 0x0a, 0x0a))?;
    }
    Ok(())
}

/// Tab-style chrome — Bedrock: 36px inactive, raised active tab 4px taller.
/// `selected` tab gets a 4px height boost (drawn 4px up). Caller draws captions.
pub fn draw_tab_strip<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    strip_top_left: Point,
    tab_height: u32,
    widths: &[u32],
    selected: usize,
    face: Rgb888,
) -> Result<(), D::Error> {
    if widths.is_empty() {
        return Ok(());
    }
    // Bedrock: inactive = blockSizes.md (36px), active = md + 4 = 40px (raised 4px)
    let h_inactive = tab_height.max(28);
    let boost = 4u32;
    let h_active = h_inactive + boost;
    let n = widths.len();
    let sel = selected.min(n.saturating_sub(1));
    let mut x = strip_top_left.x;
    let y_base = strip_top_left.y;

    // Draw inactive tabs first so active tab paints on top
    for (i, &w) in widths.iter().enumerate() {
        if i == sel {
            x += w.max(20) as i32 + 2;
            continue;
        }
        let w = w.max(20);
        let tab = Rectangle::new(Point::new(x, y_base), Size::new(w, h_inactive));
        bevel.draw_raised(target, tab)?;
        // Fill inner face (covers the inner bevel area)
        let inner = pad(tab, BedrockBevel::BEVEL_PX);
        Rectangle::new(inner.top_left, inner.size)
            .into_styled(PrimitiveStyle::with_fill(face))
            .draw(target)?;
        x += w as i32 + 2;
    }

    // Draw active tab (raised 4px above baseline)
    x = strip_top_left.x;
    for (i, &w) in widths.iter().enumerate() {
        let w = w.max(20);
        if i == sel {
            let tab = Rectangle::new(
                Point::new(x, y_base - boost as i32),
                Size::new(w, h_active),
            );
            bevel.draw_raised(target, tab)?;
            let inner = pad(tab, BedrockBevel::BEVEL_PX);
            Rectangle::new(inner.top_left, inner.size)
                .into_styled(PrimitiveStyle::with_fill(face))
                .draw(target)?;
            // Bedrock: selected tab merges with content — erase the bottom border
            // rows (3px deep) by repainting them with face color, opening the tab into
            // the panel below.
            let bx = tab.top_left.x;
            let by = tab.top_left.y + tab.size.height as i32;
            let bw = tab.size.width as i32;
            hline(target, bx + 1, bx + bw - 2, by - 1, face)?;
            hline(target, bx + 2, bx + bw - 3, by - 2, face)?;
            hline(target, bx + 3, bx + bw - 4, by - 3, face)?;
            break;
        }
        x += w as i32 + 2;
    }
    Ok(())
}

/// Scrollbar arrow button — Bedrock: 26×26 raised button with triangle glyph.
///
/// `direction`: 0=Up, 1=Down, 2=Left, 3=Right.
/// `pressed`: inverts bevel (buttonPressed style).
pub fn draw_scrollbar_arrow<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    direction: u8,
    pressed: bool,
) -> Result<(), D::Error> {
    if pressed {
        // Pressed: invert bevel (buttonPressed style)
        draw_raised_pressed(target, bevel, rect)?;
    } else {
        bevel.draw_raised(target, rect)?;
    }
    // Draw centered triangle glyph
    let cx = rect.top_left.x + rect.size.width as i32 / 2;
    let cy = rect.top_left.y + rect.size.height as i32 / 2;
    let offset = if pressed { 1 } else { 0 };
    let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
    // Triangle vertices (7px span, 4px tall) — rotated per direction
    let (p0, p1, p2) = match direction {
        0 => ( // Up
            Point::new(cx + offset, cy - 3 + offset),
            Point::new(cx - 4 + offset, cy + 3 + offset),
            Point::new(cx + 4 + offset, cy + 3 + offset),
        ),
        1 => ( // Down
            Point::new(cx + offset, cy + 3 + offset),
            Point::new(cx - 4 + offset, cy - 3 + offset),
            Point::new(cx + 4 + offset, cy - 3 + offset),
        ),
        2 => ( // Left
            Point::new(cx - 3 + offset, cy + offset),
            Point::new(cx + 3 + offset, cy - 4 + offset),
            Point::new(cx + 3 + offset, cy + 4 + offset),
        ),
        _ => ( // Right
            Point::new(cx + 3 + offset, cy + offset),
            Point::new(cx - 3 + offset, cy - 4 + offset),
            Point::new(cx - 3 + offset, cy + 4 + offset),
        ),
    };
    Triangle::new(p0, p1, p2)
        .into_styled(PrimitiveStyle::with_fill(ink))
        .draw(target)
}

/// Full vertical scrollbar: up/down arrow buttons, hatched track, raised thumb.
///
/// Bedrock / Win32 style — width is typically 26px (`SB_W`).
/// - `rect` — full scrollbar rectangle (arrows + track together).
/// - `thumb_center` — 0..=1 position of the thumb center along the track (`None` if no scroll).
/// - `thumb_len`    — 0..=1 fraction of the track the thumb covers (`None` if no scroll).
/// - `up_pressed` / `dn_pressed` — invert bevel on the arrow button that is currently held.
pub fn draw_scrollbar_vertical<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    thumb_center: Option<f32>,
    thumb_len: Option<f32>,
    up_pressed: bool,
    dn_pressed: bool,
) -> Result<(), D::Error> {
    let arrow_h = rect.size.width; // square arrow buttons (width = height)
    let up_btn = Rectangle::new(rect.top_left, Size::new(rect.size.width, arrow_h));
    let dn_btn = Rectangle::new(
        Point::new(
            rect.top_left.x,
            rect.top_left.y + rect.size.height as i32 - arrow_h as i32,
        ),
        Size::new(rect.size.width, arrow_h),
    );
    let track = Rectangle::new(
        Point::new(rect.top_left.x, rect.top_left.y + arrow_h as i32),
        Size::new(
            rect.size.width,
            rect.size.height.saturating_sub(arrow_h * 2),
        ),
    );

    draw_scrollbar_arrow(target, bevel, up_btn, 0, up_pressed)?;
    draw_scrollbar_arrow(target, bevel, dn_btn, 1, dn_pressed)?;
    draw_hatched_background(target, track, bevel.face, bevel.border_lightest)?;

    if let (Some(tc), Some(tl)) = (thumb_center, thumb_len) {
        let span = track.size.height;
        let th = ((span as f32 * tl) as u32).max(arrow_h).min(span);
        let avail = span.saturating_sub(th);
        let ty = track.top_left.y + (avail as f32 * tc.clamp(0.0, 1.0)) as i32;
        let thumb = Rectangle::new(
            Point::new(track.top_left.x, ty),
            Size::new(track.size.width, th),
        );
        bevel.draw_raised(target, thumb)?;
    }
    Ok(())
}

/// Hatched 2×2 checkerboard background — used for scrollbar tracks and pressed button state.
/// Pattern: `(x+y) % 2 == 0` → `primary`, else → `secondary`.
pub fn draw_hatched_background<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    area: Rectangle,
    primary: Rgb888,
    secondary: Rgb888,
) -> Result<(), D::Error> {
    let x0 = area.top_left.x;
    let y0 = area.top_left.y;
    let w = area.size.width as i32;
    let h = area.size.height as i32;
    for dy in 0..h {
        for dx in 0..w {
            let c = if (dx + dy) % 2 == 0 { primary } else { secondary };
            // Draw 1×1 pixel
            Line::new(
                Point::new(x0 + dx, y0 + dy),
                Point::new(x0 + dx, y0 + dy),
            )
            .into_styled(PrimitiveStyle::with_stroke(c, 1))
            .draw(target)?;
        }
    }
    Ok(())
}

/// Small centered down-pointing triangle glyph — for dropdown/combo box buttons.
/// Draws a filled triangle (7px wide, 4px tall) centered in `rect`.
pub fn draw_dropdown_glyph<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    rect: Rectangle,
    ink: Rgb888,
) -> Result<(), D::Error> {
    let cx = rect.top_left.x + rect.size.width as i32 / 2;
    let cy = rect.top_left.y + rect.size.height as i32 / 2;
    Triangle::new(
        Point::new(cx, cy + 3),
        Point::new(cx - 4, cy - 1),
        Point::new(cx + 4, cy - 1),
    )
    .into_styled(PrimitiveStyle::with_fill(ink))
    .draw(target)
}

/// Status-bar border — 1-layer: top/left = `border_dark`, bottom/right = `border_lightest`.
///
/// Thin wrapper around [`BedrockBevel::draw_status_border`].
pub fn draw_status_border<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
) -> Result<(), D::Error> {
    bevel.draw_status_border(target, rect)
}

/// Multi-segment status bar.
///
/// Draws the outer `bar` container as a sunken strip, then fills it with `face` and splits it
/// into `segments` cells separated by 1 px beveled grooves (`border_dark` left, `border_lightest`
/// right). Each segment is drawn with a `draw_status_border` inset.
///
/// `segment_widths` — pixel width of each segment. If the sum is less than the bar interior,
/// the last segment expands to fill. Pass `&[]` to draw a plain unsegmented bar.
pub fn draw_status_segments<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    bar: Rectangle,
    face: Rgb888,
    segment_widths: &[u32],
) -> Result<(), D::Error> {
    // Outer container: draw top/left inset line (status style)
    bevel.draw_status_border(target, bar)?;
    let inner = pad(bar, 1);
    // Fill background
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(face))
        .draw(target)?;
    if segment_widths.is_empty() {
        return Ok(());
    }
    // Draw each segment as an inset status-border cell
    let mut x = inner.top_left.x;
    let available_w = inner.size.width;
    let sum: u32 = segment_widths.iter().sum();
    for (i, &sw) in segment_widths.iter().enumerate() {
        let is_last = i == segment_widths.len() - 1;
        let w = if is_last {
            (inner.top_left.x + available_w as i32 - x).max(0) as u32
        } else if sum > 0 {
            sw
        } else {
            available_w / segment_widths.len() as u32
        };
        if w == 0 { break; }
        let seg = Rectangle::new(Point::new(x, inner.top_left.y), Size::new(w, inner.size.height));
        bevel.draw_status_border(target, seg)?;
        // Divider groove between segments (skip after last)
        if !is_last {
            vline(target, x + w as i32,     inner.top_left.y, inner.top_left.y + inner.size.height as i32 - 1, bevel.border_dark)?;
            vline(target, x + w as i32 + 1, inner.top_left.y, inner.top_left.y + inner.size.height as i32 - 1, bevel.border_lightest)?;
        }
        x += w as i32 + 2; // +2 for the 2-px divider
    }
    Ok(())
}

/// GroupBox etched border with optional label gap.
///
/// Thin wrapper around [`BedrockBevel::draw_groupbox`] for API consistency with the other
/// `bedrock_controls` helpers.  Pass `label_gap` as `Some((x_start, pixel_width))` to leave a blank
/// notch in the top border where the label text sits.
pub fn draw_group_box<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    label_gap: Option<(i32, i32)>,
    face: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_groupbox(target, rect, label_gap, Some(face))
}

/// Alias for [`draw_group_box`].
pub fn draw_groupbox_border<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    label_gap: Option<(i32, i32)>,
    face: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_groupbox(target, rect, label_gap, Some(face))
}

/// Horizontal etched separator — 2px: first row = `border_dark`, second = `border_lightest`.
pub fn draw_separator_h<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    x0: i32,
    x1: i32,
    y: i32,
) -> Result<(), D::Error> {
    hline(target, x0, x1, y, bevel.border_dark)?;
    hline(target, x0, x1, y + 1, bevel.border_lightest)
}

/// Vertical etched separator — 2px: first col = `border_dark`, second = `border_lightest`.
pub fn draw_separator_v<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    x: i32,
    y0: i32,
    y1: i32,
) -> Result<(), D::Error> {
    vline(target, x, y0, y1, bevel.border_dark)?;
    vline(target, x + 1, y0, y1, bevel.border_lightest)
}

/// Tooltip chrome: 1px `borderDarkest` border + cream fill (`tooltip_bg`).
pub fn draw_tooltip_chrome<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    tooltip_bg: Rgb888,
) -> Result<(), D::Error> {
    Rectangle::new(rect.top_left, rect.size)
        .into_styled(PrimitiveStyle::with_fill(tooltip_bg))
        .draw(target)?;
    rect.into_styled(PrimitiveStyle::with_stroke(bevel.border_darkest, 1))
        .draw(target)
}

/// Title bar fill for a window or dialog.
///
/// - `active = true`  → `accent` fill (navy) with `caption_on_accent` text.
/// - `active = false` → `border_dark` fill (gray) with `caption_on_accent` text (dimmed title).
pub fn draw_window_title_bar<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    rect: Rectangle,
    label: &str,
    font: &MonoFont<'_>,
    active: bool,
    accent: Rgb888,
    caption_on_accent: Rgb888,
    border_dark: Rgb888,
) -> Result<(), D::Error> {
    let fill = if active { accent } else { border_dark };
    Rectangle::new(rect.top_left, rect.size)
        .into_styled(PrimitiveStyle::with_fill(fill))
        .draw(target)?;
    let style = MonoTextStyle::new(font, caption_on_accent);
    Text::with_baseline(
        label,
        Point::new(rect.top_left.x + 8, rect.top_left.y + (rect.size.height as i32 - font.character_size.height as i32) / 2),
        style,
        Baseline::Top,
    ).draw(target)?;
    Ok(())
}

/// Title-bar button (Min / Max / Close) — Bedrock: 27×31 raised button.
/// Caller draws the icon glyph (×, −, □) on top of this chrome.
pub fn draw_title_button<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    pressed: bool,
) -> Result<(), D::Error> {
    if pressed {
        draw_raised_pressed(target, bevel, rect)
    } else {
        bevel.draw_raised(target, rect)
    }
}

/// **Pressed-button bevel** (`buttonPressed` style): inverted 4-layer border.
///
/// ```text
/// outer TL 2px : border_darkest   outer BR 2px : border_lightest
/// inner TL 1px : border_dark      inner BR 1px : border_light
/// ```
pub fn draw_raised_pressed<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
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
        .into_styled(PrimitiveStyle::with_fill(bevel.face))
        .draw(target)?;

    // Outer 2px: TL = darkest, BR = lightest (inverted from raised)
    hline(target, x, x + w - 1, y, bevel.border_darkest)?;
    hline(target, x, x + w - 1, y + 1, bevel.border_darkest)?;
    vline(target, x, y + 2, y + h - 3, bevel.border_darkest)?;
    vline(target, x + 1, y + 2, y + h - 3, bevel.border_darkest)?;
    hline(target, x, x + w - 1, y + h - 1, bevel.border_lightest)?;
    hline(target, x, x + w - 1, y + h - 2, bevel.border_lightest)?;
    vline(target, x + w - 1, y + 2, y + h - 3, bevel.border_lightest)?;
    vline(target, x + w - 2, y + 2, y + h - 3, bevel.border_lightest)?;

    // Inner 1px: TL = dark, BR = light
    hline(target, x + 2, x + w - 3, y + 2, bevel.border_dark)?;
    vline(target, x + 2, y + 3, y + h - 4, bevel.border_dark)?;
    hline(target, x + 2, x + w - 3, y + h - 3, bevel.border_light)?;
    vline(target, x + w - 3, y + 3, y + h - 4, bevel.border_light)?;

    Ok(())
}

// ── Menubar chrome ─────────────────────────────────────────────────────────────

/// Draw the horizontal menu bar strip.
///
/// - Fills `strip` with `surface`.
/// Draw a text label where the first `&` in the string marks the mnemonic character.
///
/// The `&` is stripped before rendering; the following character is underlined with a 1 px line
/// one pixel below the text baseline. `"&File"` renders as `File` with `F` underlined.
/// If the string contains no `&`, it is rendered normally.
pub fn draw_mnemonic_label<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    text: &str,
    font: &MonoFont<'_>,
    origin: Point,
    color: Rgb888,
) -> Result<(), D::Error> {
    // Split on the first '&'
    let amp = text.find('&');
    let display: alloc::string::String = text.chars().filter(|&c| c != '&').collect();
    let style = MonoTextStyle::new(font, color);
    Text::with_baseline(&display, origin, style, Baseline::Top).draw(target)?;
    if let Some(idx) = amp {
        let char_w = font.character_size.width as i32;
        let char_h = font.character_size.height as i32;
        // Count characters before the mnemonic (excluding the '&' itself)
        let prefix_chars = text[..idx].chars().count() as i32;
        let ul_x = origin.x + prefix_chars * char_w;
        let ul_y = origin.y + char_h + 1;
        hline(target, ul_x, ul_x + char_w - 1, ul_y, color)?;
    }
    Ok(())
}

/// - For each `(cell, label)` pair: fills with `accent` if `focused_i == Some(i)`, else `surface`;
///   draws the label in `caption_on_accent` / `text` accordingly.
///
/// Metrics: text at `(cell.x + MENU_ITEM_PAD_X, cell.y + MENU_ITEM_PAD_Y)`.
pub fn draw_menu_bar<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    strip: Rectangle,
    cells: &[Rectangle],
    labels: &[&str],
    focused_i: Option<usize>,
    font: &MonoFont<'_>,
    surface: Rgb888,
    text: Rgb888,
    accent: Rgb888,
    caption_on_accent: Rgb888,
) -> Result<(), D::Error> {
    Rectangle::new(strip.top_left, strip.size)
        .into_styled(PrimitiveStyle::with_fill(surface))
        .draw(target)?;
    for (i, cell) in cells.iter().enumerate() {
        let sel = focused_i == Some(i);
        let bg = if sel { accent } else { surface };
        let fg = if sel { caption_on_accent } else { text };
        Rectangle::new(cell.top_left, cell.size)
            .into_styled(PrimitiveStyle::with_fill(bg))
            .draw(target)?;
        if let Some(lbl) = labels.get(i) {
            draw_mnemonic_label(
                target,
                lbl,
                font,
                Point::new(cell.top_left.x + MENU_ITEM_PAD_X, cell.top_left.y + MENU_ITEM_PAD_Y),
                fg,
            )?;
        }
    }
    Ok(())
}

/// Draw a dropdown menu popup.
///
/// - Draws raised-soft chrome around `popup`.
/// - Fills interior (inset 3px) with `canvas`.
/// - For each item: `"—"` → etched [`draw_separator_h`]; otherwise row highlight + text.
///
/// Metrics: rows at `popup.y + MENU_POPUP_TOP_PAD + j * line_h`;
/// text at `popup.x + MENU_POPUP_PAD_X`.
pub fn draw_menu_popup<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    popup: Rectangle,
    items: &[&str],
    selected_i: usize,
    line_h: i32,
    font: &MonoFont<'_>,
    canvas: Rgb888,
    text: Rgb888,
    selection_bg: Rgb888,
    caption_on_accent: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_raised_soft(target, popup)?;
    let inner = pad(popup, 3);
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(canvas))
        .draw(target)?;
    for (j, name) in items.iter().enumerate() {
        let row_y = popup.top_left.y + MENU_POPUP_TOP_PAD + j as i32 * line_h;
        if *name == "—" {
            let sep_y = row_y + line_h / 2 - 1;
            let x0 = popup.top_left.x + MENU_POPUP_PAD_X;
            let x1 = popup.top_left.x + popup.size.width as i32 - MENU_POPUP_PAD_X;
            draw_separator_h(target, bevel, x0, x1, sep_y)?;
            continue;
        }
        let row_sel = selected_i == j;
        let bg = if row_sel { selection_bg } else { canvas };
        let fg = if row_sel { caption_on_accent } else { text };
        let rw = popup.size.width.saturating_sub(4);
        Rectangle::new(
            Point::new(popup.top_left.x + 2, row_y),
            Size::new(rw, line_h as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(bg))
        .draw(target)?;
        let font_h = font.character_size.height as i32;
        draw_mnemonic_label(
            target,
            name,
            font,
            Point::new(popup.top_left.x + MENU_POPUP_PAD_X, row_y + (line_h - font_h) / 2),
            fg,
        )?;
    }
    Ok(())
}

/// Like [`draw_menu_popup`] but with per-item disabled flags.
///
/// `disabled[j] = true` renders item `j` in the classic embossed gray style and
/// keeps it unselectable visually (the caller is responsible for not activating it).
/// If `disabled` is shorter than `items`, missing entries are treated as enabled.
pub fn draw_menu_popup_ex<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    popup: Rectangle,
    items: &[&str],
    disabled: &[bool],
    selected_i: usize,
    line_h: i32,
    font: &MonoFont<'_>,
    canvas: Rgb888,
    text: Rgb888,
    selection_bg: Rgb888,
    caption_on_accent: Rgb888,
) -> Result<(), D::Error> {
    bevel.draw_raised_soft(target, popup)?;
    let inner = pad(popup, 3);
    Rectangle::new(inner.top_left, inner.size)
        .into_styled(PrimitiveStyle::with_fill(canvas))
        .draw(target)?;
    for (j, name) in items.iter().enumerate() {
        let row_y = popup.top_left.y + MENU_POPUP_TOP_PAD + j as i32 * line_h;
        if *name == "—" {
            let sep_y = row_y + line_h / 2 - 1;
            let x0 = popup.top_left.x + MENU_POPUP_PAD_X;
            let x1 = popup.top_left.x + popup.size.width as i32 - MENU_POPUP_PAD_X;
            draw_separator_h(target, bevel, x0, x1, sep_y)?;
            continue;
        }
        let is_disabled = disabled.get(j).copied().unwrap_or(false);
        let row_sel = !is_disabled && selected_i == j;
        let bg = if row_sel { selection_bg } else { canvas };
        let rw = popup.size.width.saturating_sub(4);
        Rectangle::new(
            Point::new(popup.top_left.x + 2, row_y),
            Size::new(rw, line_h as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(bg))
        .draw(target)?;
        let font_h = font.character_size.height as i32;
        let label_pt = Point::new(popup.top_left.x + MENU_POPUP_PAD_X, row_y + (line_h - font_h) / 2);
        if is_disabled {
            draw_label_disabled(target, name, font, label_pt.x, label_pt.y, bevel.border_dark, bevel.border_lightest)?;
        } else {
            let fg = if row_sel { caption_on_accent } else { text };
            draw_mnemonic_label(target, name, font, label_pt, fg)?;
        }
    }
    Ok(())
}

// ── File picker icon glyphs ───────────────────────────────────────────────────

/// 16×16 folder icon — Bedrock style.
///
/// Draws an orange trapezoid "tab" (top 5px) + a body rectangle (bottom 10px).
/// Draw a 16×16 folder icon.  When `enabled = false` the icon is drawn twice:
/// first offset +1,+1 in `border_lightest` (white shadow), then at 0,0 in `border_dark`
/// (gray), producing the classic embossed disabled look.
pub fn draw_folder_icon<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y: i32,
    enabled: bool,
) -> Result<(), D::Error> {
    if !enabled {
        draw_folder_icon_colored(target, x + 1, y + 1, Rgb888::new(0xfe, 0xfe, 0xfe), Rgb888::new(0xdf, 0xdf, 0xdf))?;
        return draw_folder_icon_colored(target, x, y, Rgb888::new(0x84, 0x85, 0x84), Rgb888::new(0x84, 0x85, 0x84));
    }
    draw_folder_icon_colored(target, x, y, Rgb888::new(0xff, 0xcc, 0x00), Rgb888::new(0xcc, 0x99, 0x00))
}

fn draw_folder_icon_colored<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y: i32,
    body_fill: Rgb888,
    tab_fill: Rgb888,
) -> Result<(), D::Error> {
    let ink  = Rgb888::new(0x0a, 0x0a, 0x0a);
    let edge = Rgb888::new(0x84, 0x85, 0x84);
    Rectangle::new(Point::new(x, y + 3), Size::new(7, 3))
        .into_styled(PrimitiveStyle::with_fill(tab_fill))
        .draw(target)?;
    Rectangle::new(Point::new(x, y + 5), Size::new(16, 11))
        .into_styled(PrimitiveStyle::with_fill(body_fill))
        .draw(target)?;
    hline(target, x, x + 15, y + 5, ink)?;
    vline(target, x, y + 5, y + 15, ink)?;
    hline(target, x, x + 15, y + 15, edge)?;
    vline(target, x + 15, y + 5, y + 15, edge)?;
    Ok(())
}

/// 16×16 file/document icon — Bedrock style.
///
/// White page with folded top-right corner. When `enabled = false` the icon is rendered with the
/// classic embossed disabled look (white shadow +1,+1 then gray on top).
pub fn draw_file_icon<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y: i32,
    enabled: bool,
) -> Result<(), D::Error> {
    if !enabled {
        draw_file_icon_colored(target, x + 1, y + 1, Rgb888::new(0xfe, 0xfe, 0xfe), Rgb888::new(0xfe, 0xfe, 0xfe))?;
        return draw_file_icon_colored(target, x, y, Rgb888::new(0x84, 0x85, 0x84), Rgb888::new(0xc0, 0xc0, 0xc0));
    }
    draw_file_icon_colored(target, x, y, Rgb888::WHITE, Rgb888::new(0x84, 0x85, 0x84))
}

fn draw_file_icon_colored<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y: i32,
    page_fill: Rgb888,
    line_color: Rgb888,
) -> Result<(), D::Error> {
    let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
    Rectangle::new(Point::new(x, y), Size::new(12, 16))
        .into_styled(PrimitiveStyle::with_fill(page_fill))
        .draw(target)?;
    Rectangle::new(Point::new(x + 12, y + 4), Size::new(4, 12))
        .into_styled(PrimitiveStyle::with_fill(page_fill))
        .draw(target)?;
    hline(target, x, x + 11, y, ink)?;
    vline(target, x, y, y + 15, ink)?;
    hline(target, x, x + 15, y + 15, ink)?;
    vline(target, x + 15, y + 4, y + 15, ink)?;
    hline(target, x + 12, x + 15, y + 4, ink)?;
    vline(target, x + 12, y, y + 4, ink)?;
    hline(target, x + 2, x + 9, y + 5, line_color)?;
    hline(target, x + 2, x + 9, y + 8, line_color)?;
    hline(target, x + 2, x + 9, y + 11, line_color)?;
    Ok(())
}

// ── File picker layout + draw ─────────────────────────────────────────────────

/// Standard width of a vertical scrollbar in the file picker.
pub const FP_SB_W: u32 = 26;
/// Height of the title bar in the file picker dialog.
pub const FP_TITLE_H: u32 = 26;
/// Height of the "Look in" toolbar strip.
pub const FP_TOOLBAR_H: u32 = 34;
/// Height of the filename / file-type row.
pub const FP_FIELD_ROW_H: u32 = 26;
/// Height of the OK / Cancel button row + padding.
pub const FP_BUTTON_ROW_H: u32 = 38;
/// Width of OK / Cancel buttons.
pub const FP_BTN_W: u32 = 80;
/// Width of the label column ("File name:", "Files of type:").
pub const FP_LABEL_W: u32 = 90;

/// Computed geometry for the file picker dialog.
///
/// Build with [`compute_file_picker_layout`]; pass to [`draw_file_picker`].
#[derive(Debug, Clone, Copy)]
pub struct FilePickerLayout {
    pub dialog:          Rectangle,
    pub title_bar:       Rectangle,
    pub close_btn:       Rectangle,
    pub toolbar:         Rectangle,
    pub look_in_label:   Rectangle,
    pub look_in_dd:      Rectangle,
    pub nav_up_btn:      Rectangle,
    pub list_outer:      Rectangle,
    pub list_inner:      Rectangle,
    pub sb_rect:         Rectangle,
    pub filename_label:  Rectangle,
    pub filename_field:  Rectangle,
    pub filetype_label:  Rectangle,
    pub filetype_field:  Rectangle,
    pub ok_btn:          Rectangle,
    pub cancel_btn:      Rectangle,
    /// Number of rows visible in the file list (`list_inner.height / line_h`).
    pub visible_rows:    usize,
}

/// Derive all sub-rectangles from a dialog outer rect and `line_h`.
///
/// `sb_w` — scrollbar width (use [`FP_SB_W`] = 26 for Bedrock style).
pub fn compute_file_picker_layout(dialog: Rectangle, line_h: i32, sb_w: u32) -> FilePickerLayout {
    let x = dialog.top_left.x;
    let y = dialog.top_left.y;
    let dw = dialog.size.width;
    let dh = dialog.size.height;
    let bevel = BedrockBevel::BEVEL_PX as i32; // 3
    let pad = bevel + 5; // 8px inner margin for comfortable window feel

    let title_bar = Rectangle::new(Point::new(x, y), Size::new(dw, FP_TITLE_H));
    // Close button: 22×20, 3px from right, vertically centered in title bar
    let cb_w = 22i32; let cb_h = 20i32;
    let close_btn = Rectangle::new(
        Point::new(x + dw as i32 - 3 - cb_w, y + (FP_TITLE_H as i32 - cb_h) / 2),
        Size::new(cb_w as u32, cb_h as u32),
    );

    let toolbar_y = y + FP_TITLE_H as i32;
    let toolbar = Rectangle::new(Point::new(x + pad, toolbar_y), Size::new(dw - pad as u32 * 2, FP_TOOLBAR_H));
    // "Look in:" label is 56px wide inside toolbar
    let look_in_label_w = 56i32;
    let look_in_label = Rectangle::new(
        Point::new(toolbar.top_left.x + 4, toolbar_y + (FP_TOOLBAR_H as i32 - line_h) / 2),
        Size::new(look_in_label_w as u32, line_h as u32),
    );
    // Nav-up button: square, right side of toolbar
    let nav_btn = Rectangle::new(
        Point::new(toolbar.top_left.x + toolbar.size.width as i32 - sb_w as i32, toolbar_y + (FP_TOOLBAR_H as i32 - sb_w as i32) / 2),
        Size::new(sb_w, sb_w),
    );
    // Path dropdown fills between label and nav button
    let dd_x = look_in_label.top_left.x + look_in_label_w + 4;
    let dd_w = (nav_btn.top_left.x - dd_x - 4).max(0) as u32;
    let look_in_dd = Rectangle::new(
        Point::new(dd_x, toolbar_y + (FP_TOOLBAR_H as i32 - FP_FIELD_ROW_H as i32) / 2),
        Size::new(dd_w, FP_FIELD_ROW_H),
    );

    // File list area: between toolbar and the two field rows + button row
    let footer_h = FP_FIELD_ROW_H * 2 + 6 + FP_BUTTON_ROW_H;
    let list_top = toolbar_y + FP_TOOLBAR_H as i32 + pad;
    let list_h = (dh as i32 - FP_TITLE_H as i32 - FP_TOOLBAR_H as i32 - footer_h as i32 - pad * 2).max(0) as u32;
    let list_outer = Rectangle::new(
        Point::new(x + pad, list_top),
        Size::new(dw - pad as u32 * 2, list_h),
    );
    let sb_rect = Rectangle::new(
        Point::new(list_outer.top_left.x + list_outer.size.width as i32 - sb_w as i32, list_outer.top_left.y),
        Size::new(sb_w, list_h),
    );
    let list_inner = Rectangle::new(
        Point::new(list_outer.top_left.x + bevel, list_outer.top_left.y + bevel),
        Size::new(list_outer.size.width.saturating_sub(bevel as u32 * 2 + sb_w), list_h.saturating_sub(bevel as u32 * 2)),
    );
    let visible_rows = if line_h > 0 { list_inner.size.height as usize / line_h as usize } else { 0 };

    // Filename row
    let fields_top = list_top + list_h as i32 + pad;
    let filename_label = Rectangle::new(Point::new(x + pad + 4, fields_top + (FP_FIELD_ROW_H as i32 - line_h) / 2), Size::new(FP_LABEL_W, line_h as u32));
    let fn_field_x = x + pad + FP_LABEL_W as i32;
    let fn_field_w = (dw as i32 - pad - FP_LABEL_W as i32 - pad).max(0) as u32;
    let filename_field = Rectangle::new(Point::new(fn_field_x, fields_top), Size::new(fn_field_w, FP_FIELD_ROW_H));

    // File type row
    let filetype_top = fields_top + FP_FIELD_ROW_H as i32 + 4;
    let filetype_label = Rectangle::new(Point::new(x + pad + 4, filetype_top + (FP_FIELD_ROW_H as i32 - line_h) / 2), Size::new(FP_LABEL_W, line_h as u32));
    let ft_field_x = x + pad + FP_LABEL_W as i32;
    let ft_field_w = (dw as i32 - pad - FP_LABEL_W as i32 - pad).max(0) as u32;
    let filetype_field = Rectangle::new(Point::new(ft_field_x, filetype_top), Size::new(ft_field_w, FP_FIELD_ROW_H));

    // Button row (bottom-right)
    let btn_y = y + dh as i32 - FP_BUTTON_ROW_H as i32 + (FP_BUTTON_ROW_H as i32 - FP_FIELD_ROW_H as i32) / 2;
    let cancel_btn = Rectangle::new(
        Point::new(x + dw as i32 - pad - FP_BTN_W as i32, btn_y),
        Size::new(FP_BTN_W, FP_FIELD_ROW_H),
    );
    let ok_btn = Rectangle::new(
        Point::new(cancel_btn.top_left.x - FP_BTN_W as i32 - 8, btn_y),
        Size::new(FP_BTN_W, FP_FIELD_ROW_H),
    );

    FilePickerLayout {
        dialog, title_bar, close_btn, toolbar,
        look_in_label, look_in_dd, nav_up_btn: nav_btn,
        list_outer, list_inner, sb_rect,
        filename_label, filename_field,
        filetype_label, filetype_field,
        ok_btn, cancel_btn,
        visible_rows,
    }
}

/// Which zone of the file picker has focus — re-exported from [`crate::file_picker`].
pub use crate::file_picker::FilePickerFocus;

/// Draw the complete file picker dialog (Bedrock Open/Save As style).
///
/// - `picker`          — navigation state + entry list + scroll position
/// - `filename_text`   — current text in the filename field
/// - `filetype_labels` — dropdown options (e.g. `&["All files (*.*)", "Text (*.txt)"]`)
/// - `filetype_sel`    — selected index in `filetype_labels`
/// - `open_mode`       — `true` → OK button reads "Open"; `false` → "Save"
/// - `focused`         — which zone shows a focus ring
/// - `scroll`          — scrollbar state (build from `FilePickerState` + `visible_rows`)
pub fn draw_file_picker<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    layout: &FilePickerLayout,
    picker: &crate::file_picker::FilePickerState,
    filename_text: &str,
    filetype_labels: &[&str],
    filetype_sel: usize,
    open_mode: bool,
    focused: FilePickerFocus,
    line_h: i32,
    char_w: i32,
    font: &MonoFont<'_>,
    colors: &crate::theme::ThemeColors,
    scroll: &crate::widgets::ScrollbarState,
) -> Result<(), D::Error> {
    // Dialog chrome
    bevel.draw_raised_soft(target, layout.dialog)?;

    // Title bar
    Rectangle::new(layout.title_bar.top_left, layout.title_bar.size)
        .into_styled(PrimitiveStyle::with_fill(colors.accent))
        .draw(target)?;
    let title = if open_mode { "Open" } else { "Save As" };
    Text::with_baseline(
        title,
        Point::new(layout.title_bar.top_left.x + 8, layout.title_bar.top_left.y + 7),
        MonoTextStyle::new(font, colors.caption_on_accent),
        Baseline::Top,
    ).draw(target)?;
    draw_title_button(target, bevel, layout.close_btn, false)?;
    {
        use embedded_graphics::primitives::Line;
        let cx = layout.close_btn.top_left.x + layout.close_btn.size.width as i32 / 2;
        let cy = layout.close_btn.top_left.y + layout.close_btn.size.height as i32 / 2;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
        Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
    }

    // Toolbar strip
    Rectangle::new(layout.toolbar.top_left, layout.toolbar.size)
        .into_styled(PrimitiveStyle::with_fill(colors.surface))
        .draw(target)?;
    Text::with_baseline(
        "Look in:",
        Point::new(layout.look_in_label.top_left.x, layout.look_in_label.top_left.y),
        MonoTextStyle::new(font, colors.text),
        Baseline::Top,
    ).draw(target)?;
    // Path dropdown (show current path as joined string)
    {
        let path_str: alloc::string::String = if picker.path.is_empty() {
            alloc::string::String::from("/")
        } else {
            alloc::string::String::from("/") + &picker.path.join("/")
        };
        let dd_inner = pad(layout.look_in_dd, BedrockBevel::BEVEL_PX);
        // Dropdown = text field + arrow button
        let arrow_w = FP_SB_W;
        let field_w = layout.look_in_dd.size.width.saturating_sub(arrow_w);
        let field = Rectangle::new(layout.look_in_dd.top_left, Size::new(field_w, layout.look_in_dd.size.height));
        let arrow_btn = Rectangle::new(
            Point::new(layout.look_in_dd.top_left.x + field_w as i32, layout.look_in_dd.top_left.y),
            Size::new(arrow_w, layout.look_in_dd.size.height),
        );
        draw_combobox_chrome(target, bevel, field, arrow_btn, colors.canvas)?;
        let font_h = font.character_size.height as i32;
        let text_y = dd_inner.top_left.y + (dd_inner.size.height as i32 - font_h) / 2;
        Text::with_baseline(
            &path_str,
            Point::new(dd_inner.top_left.x + 2, text_y),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
        draw_dropdown_glyph(target, arrow_btn, colors.text)?;
    }
    // Nav-up button (↑ arrow)
    bevel.draw_raised(target, layout.nav_up_btn)?;
    {
        let cx = layout.nav_up_btn.top_left.x + layout.nav_up_btn.size.width as i32 / 2;
        let cy = layout.nav_up_btn.top_left.y + layout.nav_up_btn.size.height as i32 / 2;
        Text::with_baseline(
            "^",
            Point::new(cx - char_w / 2, cy - line_h / 2),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
    }

    // File list
    draw_sunken_field(target, bevel, layout.list_outer, colors.canvas)?;
    // Clip rows to list_inner
    for (row_offset, entry) in picker.entries.iter().skip(picker.scroll_top).take(layout.visible_rows).enumerate() {
        let row_y = layout.list_inner.top_left.y + row_offset as i32 * line_h;
        let row_idx = picker.scroll_top + row_offset;
        let sel = row_idx == picker.selected;
        let bg = if sel { colors.selection_bg } else { colors.canvas };
        let fg = if sel { colors.caption_on_accent } else { colors.text };
        // Row highlight
        Rectangle::new(
            Point::new(layout.list_inner.top_left.x, row_y),
            Size::new(layout.list_inner.size.width, line_h as u32),
        ).into_styled(PrimitiveStyle::with_fill(bg)).draw(target)?;
        // Icon (16×16), vertically centered in row
        let icon_x = layout.list_inner.top_left.x + 2;
        let icon_y = row_y + (line_h - 16) / 2;
        if entry.is_dir {
            draw_folder_icon(target, icon_x, icon_y, true)?;
        } else {
            draw_file_icon(target, icon_x, icon_y, true)?;
        }
        // Name label (after icon + 2px gap), vertically centered in row
        let font_h = font.character_size.height as i32;
        Text::with_baseline(
            &entry.name,
            Point::new(icon_x + 18, row_y + (line_h - font_h) / 2),
            MonoTextStyle::new(font, fg),
            Baseline::Top,
        ).draw(target)?;
    }
    // Scrollbar
    draw_scrollbar_vertical(
        target, bevel, layout.sb_rect,
        scroll.thumb_center_ratio(), scroll.thumb_length_ratio(),
        false, false,
    )?;
    // Focus ring on list
    if focused == FilePickerFocus::List {
        Rectangle::new(layout.list_outer.top_left, layout.list_outer.size)
            .into_styled(PrimitiveStyle::with_stroke(colors.border_focus, 1))
            .draw(target)?;
    }

    // Filename field
    Text::with_baseline(
        "File name:",
        Point::new(layout.filename_label.top_left.x, layout.filename_label.top_left.y),
        MonoTextStyle::new(font, colors.text),
        Baseline::Top,
    ).draw(target)?;
    draw_sunken_field(target, bevel, layout.filename_field, colors.canvas)?;
    {
        let fi = pad(layout.filename_field, BedrockBevel::BEVEL_PX);
        let font_h = font.character_size.height as i32;
        let text_y = fi.top_left.y + (fi.size.height as i32 - font_h) / 2;
        Text::with_baseline(
            filename_text,
            Point::new(fi.top_left.x + 2, text_y),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
    }
    if focused == FilePickerFocus::FilenameField {
        Rectangle::new(layout.filename_field.top_left, layout.filename_field.size)
            .into_styled(PrimitiveStyle::with_stroke(colors.border_focus, 1))
            .draw(target)?;
    }

    // File type dropdown
    Text::with_baseline(
        "Files of type:",
        Point::new(layout.filetype_label.top_left.x, layout.filetype_label.top_left.y),
        MonoTextStyle::new(font, colors.text),
        Baseline::Top,
    ).draw(target)?;
    {
        let arrow_w = FP_SB_W;
        let field_w = layout.filetype_field.size.width.saturating_sub(arrow_w);
        let field = Rectangle::new(layout.filetype_field.top_left, Size::new(field_w, layout.filetype_field.size.height));
        let arrow_btn = Rectangle::new(
            Point::new(layout.filetype_field.top_left.x + field_w as i32, layout.filetype_field.top_left.y),
            Size::new(arrow_w, layout.filetype_field.size.height),
        );
        draw_combobox_chrome(target, bevel, field, arrow_btn, colors.canvas)?;
        let lbl = filetype_labels.get(filetype_sel).copied().unwrap_or("");
        let fi = pad(field, BedrockBevel::BEVEL_PX);
        let font_h = font.character_size.height as i32;
        let text_y = fi.top_left.y + (fi.size.height as i32 - font_h) / 2;
        Text::with_baseline(
            lbl,
            Point::new(fi.top_left.x + 2, text_y),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
        draw_dropdown_glyph(target, arrow_btn, colors.text)?;
    }
    if focused == FilePickerFocus::FiletypeDropdown {
        Rectangle::new(layout.filetype_field.top_left, layout.filetype_field.size)
            .into_styled(PrimitiveStyle::with_stroke(colors.border_focus, 1))
            .draw(target)?;
    }

    // OK / Cancel buttons
    {
        let ok_lbl  = if open_mode { "Open" } else { "Save" };
        let pressed_ok     = focused == FilePickerFocus::OkButton;
        let pressed_cancel = focused == FilePickerFocus::CancelButton;
        // OK is the default button — draw outer ring then bevel inside
        layout.ok_btn.into_styled(PrimitiveStyle::with_stroke(bevel.border_darkest, 1)).draw(target)?;
        let ok_inner = pad(layout.ok_btn, 1);
        if pressed_ok { draw_raised_pressed(target, bevel, ok_inner)?; }
        else          { bevel.draw_raised(target, ok_inner)?; }
        let off = if pressed_ok { 1 } else { 0 };
        let ok_lbl_x = layout.ok_btn.top_left.x + (layout.ok_btn.size.width as i32 - ok_lbl.len() as i32 * char_w) / 2 + off;
        Text::with_baseline(
            ok_lbl,
            Point::new(ok_lbl_x, layout.ok_btn.top_left.y + (layout.ok_btn.size.height as i32 - line_h) / 2 + off),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;

        if pressed_cancel { draw_raised_pressed(target, bevel, layout.cancel_btn)?; }
        else              { bevel.draw_raised(target, layout.cancel_btn)?; }
        let off = if pressed_cancel { 1 } else { 0 };
        let cx = layout.cancel_btn.top_left.x + (layout.cancel_btn.size.width as i32 - "Cancel".len() as i32 * char_w) / 2 + off;
        Text::with_baseline(
            "Cancel",
            Point::new(cx, layout.cancel_btn.top_left.y + (layout.cancel_btn.size.height as i32 - line_h) / 2 + off),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
    }

    Ok(())
}

// ── TreeView ──────────────────────────────────────────────────────────────────

/// Indent width per tree level in pixels — matches Bedrock `padding-left: 19.5px`.
pub const TREE_INDENT_W: i32 = 19;
/// X offset of the vertical connector line within each indent cell.
pub const TREE_LINE_X: i32 = 9;
/// Width of the expand/collapse box (Bedrock: 8px).
pub const TREE_BOX_W: i32 = 8;
/// Height of the expand/collapse box (Bedrock: 9px).
pub const TREE_BOX_H: i32 = 9;
/// X offset from indent origin to the folder icon.
pub const TREE_ICON_X: i32 = 18;
/// X offset from indent origin to the label (icon 16px + 2px gap).
pub const TREE_LABEL_X: i32 = 36;

/// Draw a Bedrock-style tree view (Bedrock `TreeView`) inside `rect`.
///
/// Renders the visible rows from `tree.flat_rows()` starting at `tree.scroll_top`,
/// up to `visible_rows` = `rect.height / line_h`.  Each row gets:
/// - Dashed vertical connector lines for active ancestor levels.
/// - A dashed horizontal connector to the item.
/// - An expand/collapse `+`/`-` box (if the node has children).
/// - A 16×16 folder icon.
/// - The node label, highlighted with `selection_bg` + `caption_on_accent` if selected.
///
/// Pass a [`crate::widgets::ScrollbarState`] built from `tree.flat_row_count()` to
/// [`draw_scrollbar_vertical`] beside this widget for a scrollable pane.
pub fn draw_tree_view<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    rect: Rectangle,
    tree: &crate::tree_view::TreeViewState,
    line_h: i32,
    font: &MonoFont<'_>,
    colors: &crate::theme::ThemeColors,
) -> Result<(), D::Error> {
    // Sunken container
    draw_sunken_field(target, bevel, rect, colors.canvas)?;
    let inner = pad(rect, BedrockBevel::BEVEL_PX);
    let visible_rows = if line_h > 0 { inner.size.height as usize / line_h as usize } else { 0 };

    let rows = tree.flat_rows();
    let dark = colors.border;  // dashed connector color (#848584 in Bedrock theme)

    for (vi, row) in rows.iter().skip(tree.scroll_top).take(visible_rows).enumerate() {
        let row_y   = inner.top_left.y + vi as i32 * line_h;
        let row_mid = row_y + line_h / 2;
        let indent_base = inner.top_left.x + row.level as i32 * TREE_INDENT_W;

        // ── Connector lines ──────────────────────────────────────────────────
        // Vertical lines for each ancestor level that continues
        for lvl in 0..=(row.level as u32) {
            let lx = inner.top_left.x + lvl as i32 * TREE_INDENT_W + TREE_LINE_X;
            let continues = (row.continues_mask >> lvl) & 1 == 1;
            if lvl < row.level as u32 {
                // Ancestor level: draw full-height dashed segment if line continues
                if continues {
                    dashed_vline(target, lx, row_y, row_y + line_h - 1, dark)?;
                }
            } else {
                // Own level: draw half-height down from top to mid (entry connector)
                dashed_vline(target, lx, row_y, row_mid, dark)?;
                // And continue below mid if not last sibling
                if continues {
                    dashed_vline(target, lx, row_mid, row_y + line_h - 1, dark)?;
                }
                // Horizontal connector from vertical line to item
                dashed_hline(target, lx, indent_base + TREE_ICON_X - 2, row_mid, dark)?;
            }
        }

        // ── Expand/collapse box ──────────────────────────────────────────────
        if row.has_children {
            let bx = indent_base - TREE_BOX_W / 2 - 1;
            let by = row_mid - TREE_BOX_H / 2;
            // White fill + dark border
            Rectangle::new(Point::new(bx, by), Size::new(TREE_BOX_W as u32, TREE_BOX_H as u32))
                .into_styled(PrimitiveStyle::with_fill(colors.canvas))
                .draw(target)?;
            Rectangle::new(Point::new(bx, by), Size::new(TREE_BOX_W as u32, TREE_BOX_H as u32))
                .into_styled(PrimitiveStyle::with_stroke(dark, 1))
                .draw(target)?;
            // + or - glyph (horizontal bar always present)
            hline(target, bx + 2, bx + TREE_BOX_W - 2, row_mid, colors.text)?;
            if !row.expanded {
                // vertical bar for '+'
                vline(target, bx + TREE_BOX_W / 2, by + 2, by + TREE_BOX_H - 2, colors.text)?;
            }
        }

        // ── Folder icon ──────────────────────────────────────────────────────
        let icon_x = indent_base + TREE_ICON_X;
        let icon_y = row_mid - 8;
        draw_folder_icon(target, icon_x, icon_y, true)?;

        // ── Row highlight + label ────────────────────────────────────────────
        let label_x = indent_base + TREE_LABEL_X;
        let label_w  = (inner.top_left.x + inner.size.width as i32 - label_x).max(0) as u32;
        if row.selected && label_w > 0 {
            Rectangle::new(
                Point::new(label_x, row_y),
                Size::new(label_w, line_h as u32),
            )
            .into_styled(PrimitiveStyle::with_fill(colors.selection_bg))
            .draw(target)?;
        }
        let fg = if row.selected { colors.caption_on_accent } else { colors.text };
        Text::with_baseline(
            &row.label,
            Point::new(label_x + 2, row_y + (line_h - font.character_size.height as i32) / 2),
            MonoTextStyle::new(font, fg),
            Baseline::Top,
        )
        .draw(target)?;
    }
    Ok(())
}

/// Draw alternating pixels along a vertical segment — simulates Bedrock dashed connectors.
fn dashed_vline<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x: i32,
    y0: i32,
    y1: i32,
    color: Rgb888,
) -> Result<(), D::Error> {
    let mut y = y0;
    while y <= y1 {
        if (y - y0) % 2 == 0 {
            target.draw_iter(core::iter::once(
                embedded_graphics::Pixel(Point::new(x, y), color),
            ))?;
        }
        y += 1;
    }
    Ok(())
}

/// Draw alternating pixels along a horizontal segment — simulates Bedrock dashed connectors.
fn dashed_hline<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    x0: i32,
    x1: i32,
    y: i32,
    color: Rgb888,
) -> Result<(), D::Error> {
    let mut x = x0;
    while x <= x1 {
        if (x - x0) % 2 == 0 {
            target.draw_iter(core::iter::once(
                embedded_graphics::Pixel(Point::new(x, y), color),
            ))?;
        }
        x += 1;
    }
    Ok(())
}

// ── Keyboard Layout Picker layout + draw ──────────────────────────────

/// Standard width of a vertical scrollbar in the keyboard layout picker.
pub const KBD_SB_W: u32 = 26;
#[cfg(feature = "uefi")]
/// Height of the title bar in the keyboard layout picker dialog.
pub const KBD_TITLE_H: u32 = 26;
#[cfg(feature = "uefi")]
/// Height of the OK / Cancel button row + padding.
pub const KBD_BUTTON_ROW_H: u32 = 38;
#[cfg(feature = "uefi")]
/// Width of OK / Cancel buttons.
pub const KBD_BTN_W: u32 = 80;

#[cfg(feature = "uefi")]
/// Computed geometry for the keyboard layout picker dialog.
///
/// Build with [`compute_keyboard_layout_picker_layout`]; pass to [`draw_keyboard_layout_picker`].
#[derive(Debug, Clone, Copy)]
pub struct KeyboardLayoutPickerLayout {
    pub dialog: Rectangle,
    pub title_bar: Rectangle,
    pub close_btn: Rectangle,
    pub list_outer: Rectangle,
    pub list_inner: Rectangle,
    pub sb_rect: Rectangle,
    pub ok_btn: Rectangle,
    pub cancel_btn: Rectangle,
    /// Number of rows visible in the layout list (`list_inner.height / line_h`).
    pub visible_rows: usize,
}

#[cfg(feature = "uefi")]
/// Derive all sub-rectangles from a dialog outer rect and `line_h`.
///
/// `sb_w` — scrollbar width (use [`KBD_SB_W`] = 26 for Bedrock style).
pub fn compute_keyboard_layout_picker_layout(
    dialog: Rectangle,
    line_h: i32,
    sb_w: u32,
) -> KeyboardLayoutPickerLayout {
    let x = dialog.top_left.x;
    let y = dialog.top_left.y;
    let dw = dialog.size.width;
    let dh = dialog.size.height;
    let bevel_px = BedrockBevel::BEVEL_PX as i32; // 3
    let inner_pad = bevel_px + 5; // 8px inner margin

    let title_bar = Rectangle::new(Point::new(x, y), Size::new(dw, KBD_TITLE_H));
    // Close button: 22x20, 3px from right, vertically centered in title bar
    let cb_w = 22i32;
    let cb_h = 20i32;
    let close_btn = Rectangle::new(
        Point::new(x + dw as i32 - 3 - cb_w, y + (KBD_TITLE_H as i32 - cb_h) / 2),
        Size::new(cb_w as u32, cb_h as u32),
    );

    // List area: between title bar and button row
    let list_top = y + KBD_TITLE_H as i32 + inner_pad;
    // Button Y position: bottom of dialog with margin for button height
    let btn_y = y + dh as i32 - KBD_BUTTON_ROW_H as i32 + (KBD_BUTTON_ROW_H as i32 - 22) / 2;
    // List extends to just above the button area (no gap between list and buttons)
    let list_h = (btn_y - list_top).max(0) as u32;
    let list_outer = Rectangle::new(
        Point::new(x + inner_pad, list_top),
        Size::new(dw - inner_pad as u32 * 2, list_h),
    );
    let list_inner = crate::layout::pad(list_outer, BedrockBevel::BEVEL_PX);
    let sb_rect = Rectangle::new(
        Point::new(
            list_outer.top_left.x + list_outer.size.width as i32 - sb_w as i32,
            list_outer.top_left.y,
        ),
        Size::new(sb_w, list_h),
    );

    // Button row (bottom) - match file picker: right-aligned, OK then Cancel
    let cancel_btn = Rectangle::new(
        Point::new(x + dw as i32 - inner_pad - KBD_BTN_W as i32, btn_y),
        Size::new(KBD_BTN_W, 22),
    );
    let ok_btn = Rectangle::new(
        Point::new(cancel_btn.top_left.x - KBD_BTN_W as i32 - 8, btn_y),
        Size::new(KBD_BTN_W, 22),
    );

    let visible_rows = (list_h as i32 / line_h.max(1)) as usize;

    KeyboardLayoutPickerLayout {
        dialog,
        title_bar,
        close_btn,
        list_outer,
        list_inner,
        sb_rect,
        ok_btn,
        cancel_btn,
        visible_rows,
    }
}

#[cfg(feature = "uefi")]
/// Draw the keyboard layout picker dialog.
///
/// - `target` — framebuffer to draw to
/// - `bevel` — Bedrock bevel style
/// - `layout` — pre-computed geometry from `compute_keyboard_layout_picker_layout`
/// - `state` — picker state with layouts and selection
/// - `scroll` — scrollbar state
/// - `colors` — Bedrock theme colors
/// - `font` — mono font for text
/// - `line_h` — line height in pixels
pub fn draw_keyboard_layout_picker<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    layout: &KeyboardLayoutPickerLayout,
    state: &crate::keyboard_layout::KeyboardLayoutPickerState,
    scroll: &crate::widgets::ScrollbarState,
    colors: &crate::theme::ThemeColors,
    font: &MonoFont<'_>,
    line_h: i32,
) -> Result<(), D::Error> {
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
    use embedded_graphics::text::{Baseline, Text};

    // Dialog chrome
    bevel.draw_raised_soft(target, layout.dialog)?;

    // Title bar
    Rectangle::new(layout.title_bar.top_left, layout.title_bar.size)
        .into_styled(PrimitiveStyle::with_fill(colors.accent))
        .draw(target)?;
    Text::with_baseline(
        "Keyboard Layout",
        Point::new(layout.title_bar.top_left.x + 8, layout.title_bar.top_left.y + 7),
        MonoTextStyle::new(font, colors.caption_on_accent),
        Baseline::Top,
    ).draw(target)?;

    // Close button (X)
    draw_title_button(target, bevel, layout.close_btn, false)?;
    {
        let cx = layout.close_btn.top_left.x + layout.close_btn.size.width as i32 / 2;
        let cy = layout.close_btn.top_left.y + layout.close_btn.size.height as i32 / 2;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
        Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
    }

    // Layout list - draw with focus border if focused
    if state.list_focused() {
        // Draw list with selection focus indication (dashed border)
        Rectangle::new(layout.list_outer.top_left, layout.list_outer.size)
            .into_styled(PrimitiveStyle::with_stroke(colors.text, 1))
            .draw(target)?;
        draw_sunken_field(target, bevel, layout.list_outer, colors.canvas)?;
    } else {
        draw_sunken_field(target, bevel, layout.list_outer, colors.canvas)?;
    }

    // Draw list items
    let font_h = font.character_size.height as i32;
    if state.layouts.is_empty() {
        // Display "No keyboard layouts found" message centered in list area
        let msg = "No keyboard layouts found";
        let msg_w = msg.len() as i32 * font.character_size.width as i32;
        let msg_x = layout.list_inner.top_left.x + (layout.list_inner.size.width as i32 - msg_w) / 2;
        let msg_y = layout.list_inner.top_left.y + (layout.list_inner.size.height as i32 - font_h) / 2;
        Text::with_baseline(
            msg,
            Point::new(msg_x, msg_y),
            MonoTextStyle::new(font, colors.text_disabled),
            Baseline::Top,
        ).draw(target)?;
    } else {
        for (row_offset, kbd_layout) in state.layouts.iter().skip(state.scroll_top).take(layout.visible_rows).enumerate() {
            let row_y = layout.list_inner.top_left.y + row_offset as i32 * line_h;
            let row_idx = state.scroll_top + row_offset;
            let sel = row_idx == state.selected;
            let bg = if sel { colors.selection_bg } else { colors.canvas };
            let fg = if sel { colors.caption_on_accent } else { colors.text };

            // Row background
            Rectangle::new(
                Point::new(layout.list_inner.top_left.x, row_y),
                Size::new(layout.list_inner.size.width, line_h as u32),
            ).into_styled(PrimitiveStyle::with_fill(bg)).draw(target)?;

            // Layout descriptor text
            let text_y = row_y + (line_h - font_h) / 2;
            Text::with_baseline(
                &kbd_layout.descriptor,
                Point::new(layout.list_inner.top_left.x + 4, text_y),
                MonoTextStyle::new(font, fg),
                Baseline::Top,
            ).draw(target)?;
        }
    }

    // Scrollbar
    draw_scrollbar_vertical(
        target, bevel, layout.sb_rect,
        scroll.thumb_center_ratio(), scroll.thumb_length_ratio(),
        false, false,
    )?;

    // OK button - draw sunken if focused
    if state.ok_focused() {
        bevel.draw_sunken(target, layout.ok_btn)?;
        Text::with_baseline(
            "OK",
            Point::new(
                layout.ok_btn.top_left.x + layout.ok_btn.size.width as i32 / 2 - 12,
                layout.ok_btn.top_left.y + layout.ok_btn.size.height as i32 / 2 - font_h / 2,
            ),
            MonoTextStyle::new(font, colors.caption_on_accent),
            Baseline::Top,
        ).draw(target)?;
    } else {
        bevel.draw_raised(target, layout.ok_btn)?;
        Text::with_baseline(
            "OK",
            Point::new(
                layout.ok_btn.top_left.x + layout.ok_btn.size.width as i32 / 2 - 12,
                layout.ok_btn.top_left.y + layout.ok_btn.size.height as i32 / 2 - font_h / 2,
            ),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
    }

    // Cancel button - draw sunken if focused
    if state.cancel_focused() {
        bevel.draw_sunken(target, layout.cancel_btn)?;
        Text::with_baseline(
            "Cancel",
            Point::new(
                layout.cancel_btn.top_left.x + layout.cancel_btn.size.width as i32 / 2 - 20,
                layout.cancel_btn.top_left.y + layout.cancel_btn.size.height as i32 / 2 - font_h / 2,
            ),
            MonoTextStyle::new(font, colors.caption_on_accent),
            Baseline::Top,
        ).draw(target)?;
    } else {
        bevel.draw_raised(target, layout.cancel_btn)?;
        Text::with_baseline(
            "Cancel",
            Point::new(
                layout.cancel_btn.top_left.x + layout.cancel_btn.size.width as i32 / 2 - 20,
                layout.cancel_btn.top_left.y + layout.cancel_btn.size.height as i32 / 2 - font_h / 2,
            ),
            MonoTextStyle::new(font, colors.text),
            Baseline::Top,
        ).draw(target)?;
    }

    Ok(())
}

// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framebuffer::BgrxFramebuffer;

    #[test]
    fn checkbox_radio_slider_smoke() {
        let mut buf = vec![0u8; 240 * 120 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 240, 120, 240 * 4).expect("fb");
        let bevel = BedrockBevel::CLASSIC;

        // 20×20 checkbox
        let r = Rectangle::new(Point::new(4, 4), Size::new(20, 20));
        draw_checkbox_classic(&mut fb, &bevel, r, true, Rgb888::WHITE, true).unwrap();

        // 3 radio buttons (20px each + 8px gap)
        let row = Rectangle::new(Point::new(4, 30), Size::new(92, 22));
        draw_radio_row(&mut fb, row, 3, 1, &bevel, true).unwrap();

        // Slider
        let tr = Rectangle::new(Point::new(4, 58), Size::new(160, 20));
        draw_slider_track_thumb(&mut fb, &bevel, tr, 0.4, 12).unwrap();

        // Progress
        let pr = Rectangle::new(Point::new(4, 84), Size::new(160, 20));
        draw_progress_bar(&mut fb, &bevel, pr, 0.5,
            Rgb888::WHITE, Rgb888::new(0x06, 0x00, 0x84)).unwrap();

        // Tab strip
        draw_tab_strip(&mut fb, &bevel, Point::new(4, 110), 36,
            &[50, 50, 50], 1, Rgb888::new(0xc6, 0xc6, 0xc6)).unwrap();
    }

    #[test]
    fn scrollbar_arrow_smoke() {
        let mut buf = vec![0u8; 120 * 120 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 120, 120, 120 * 4).expect("fb");
        let bevel = BedrockBevel::CLASSIC;
        for dir in 0..4u8 {
            let r = Rectangle::new(
                Point::new(2 + dir as i32 * 30, 2), Size::new(26, 26));
            draw_scrollbar_arrow(&mut fb, &bevel, r, dir, false).unwrap();
        }
    }

    #[test]
    fn tooltip_separator_smoke() {
        let mut buf = vec![0u8; 200 * 60 * 4];
        let mut fb = BgrxFramebuffer::new(&mut buf, 200, 60, 200 * 4).expect("fb");
        let bevel = BedrockBevel::CLASSIC;
        let r = Rectangle::new(Point::new(4, 4), Size::new(120, 28));
        draw_tooltip_chrome(&mut fb, &bevel, r, Rgb888::new(0xfe, 0xfb, 0xcc)).unwrap();
        draw_separator_h(&mut fb, &bevel, 4, 180, 40).unwrap();
    }
}
