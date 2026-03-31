//! Full-window painting for the firmware demo (same code path as [`crate::paint_demo_snapshot`] on host).

use alloc::string::String;
use alloc::vec::Vec;

use fontdue::Font;
use uefi_ui::embedded_graphics::geometry::Point;
use uefi_ui::embedded_graphics::pixelcolor::Rgb888;
use uefi_ui::embedded_graphics::prelude::*;
use uefi_ui::embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use uefi_ui::embedded_graphics::text::{Baseline, Text};
use uefi_ui::embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::bedrock_controls::{draw_button, draw_menu_bar, draw_menu_popup, draw_scrollbar_vertical, draw_title_button, draw_window_title_bar};
use uefi_ui::png::{decode_png_to_rgba, png_dimensions_and_size};
use uefi_ui::popover::{center_in_screen, PopoverStack};
use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::theme::Theme;
use uefi_ui::widgets::{
    NavBar, ScrollAxis, ScrollbarState, TextArea,
};
use uefi_ui::window::{WindowOffset, WindowStack};

use crate::demo_gallery::{paint_gallery, GalleryState};
use crate::layout::{submenu_popup_rect, Focus, UiLayout, SUBMENUS};
use crate::ttf_text;

/// Same assets as the firmware binary (`include_bytes!` is rooted at this crate).
pub static DEMO_PNG_BYTES: &[u8] = include_bytes!("../../../assets/images/test.png");

/// Decode `test.png`; cap buffer to avoid huge allocations on bad input.
pub fn decode_demo_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let (_w, _h, need) = png_dimensions_and_size(bytes).ok()?;
    if need > 16 * 1024 * 1024 {
        return None;
    }
    let mut buf = Vec::new();
    buf.try_reserve_exact(need).ok()?;
    buf.resize(need, 0);
    let img = decode_png_to_rgba(bytes, &mut buf).ok()?;
    let pw = img.width();
    let ph = img.height();
    Some((buf, pw, ph))
}

/// Scale `rgba` (8-bit RGBA) into `frame` with letterboxing; alpha-composite onto `target`.
pub fn blit_rgba_contain(
    target: &mut BgrxFramebuffer<'_>,
    rgba: &[u8],
    src_w: u32,
    src_h: u32,
    frame: Rectangle,
) {
    let dw = frame.size.width;
    let dh = frame.size.height;
    if dw == 0 || dh == 0 || src_w == 0 || src_h == 0 {
        return;
    }
    let s = (dw as f32 / src_w as f32).min(dh as f32 / src_h as f32);
    let out_w = ((src_w as f32) * s) as u32;
    let out_h = ((src_h as f32) * s) as u32;
    if out_w == 0 || out_h == 0 {
        return;
    }
    let ox = frame.top_left.x + (frame.size.width as i32 - out_w as i32) / 2;
    let oy = frame.top_left.y + (frame.size.height as i32 - out_h as i32) / 2;
    let len = rgba.len();
    for dy in 0..out_h {
        for dx in 0..out_w {
            let sx = (((dx as f32 + 0.5) / s) as u32).min(src_w.saturating_sub(1));
            let sy = (((dy as f32 + 0.5) / s) as u32).min(src_h.saturating_sub(1));
            let i = ((sy * src_w + sx) * 4) as usize;
            if i + 3 >= len {
                continue;
            }
            let r = rgba[i];
            let g = rgba[i + 1];
            let b = rgba[i + 2];
            let a = rgba[i + 3];
            if a == 0 {
                continue;
            }
            let px = ox + dx as i32;
            let py = oy + dy as i32;
            if px < 0 || py < 0 {
                continue;
            }
            let fg = Rgb888::new(r, g, b);
            target.blend_pixel(px as u32, py as u32, fg, a);
        }
    }
}

fn cursor_line_col(ta: &TextArea) -> (usize, usize) {
    let b = ta.cursor().min(ta.text.len());
    let t = ta.text.as_str();
    let line_idx = t.as_bytes()[..b].iter().filter(|&&x| x == b'\n').count();
    let line_start = t[..b].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let col = t[line_start..b].chars().count();
    (line_idx, col)
}

fn caret_xy(
    ta: &TextArea,
    inner: &Rectangle,
    line_h: i32,
    char_w: i32,
) -> (i32, i32) {
    let (line_idx, col) = cursor_line_col(ta);
    let rel = line_idx.saturating_sub(ta.scroll_top_line);
    let cx = inner.top_left.x + 4 + col as i32 * char_w;
    let cy = inner.top_left.y + rel as i32 * line_h;
    (cx, cy)
}

/// Paint the full interactive demo window (no mouse cursor sprite).
pub fn paint_scene_no_cursor(
    target: &mut BgrxFramebuffer<'_>,
    w: u32,
    h: u32,
    theme: &Theme,
    bevel: &BedrockBevel,
    font: &MonoFont<'_>,
    ttf: Option<&Font>,
    nav: &NavBar<'_>,
    gallery: &GalleryState,
    textarea: &TextArea,
    png_rgba: Option<(&[u8], u32, u32)>,
    focus: Focus,
    scroll: &ScrollbarState,
    line_h: i32,
    char_w: i32,
    layout: &UiLayout,
    popovers: &PopoverStack,
    win_stack: &WindowStack,
) {
    target.fill_rect_solid(0, 0, w, h, theme.colors.background);

    let _ = bevel.draw_raised_soft(target, layout.win);
    Rectangle::new(layout.title_bar.top_left, layout.title_bar.size)
        .into_styled(PrimitiveStyle::with_fill(theme.colors.accent))
        .draw(target)
        .ok();
    let title_txt = "uefi_ui demo";
    if let Some(f) = ttf {
        let bx = layout.title_bar.top_left.x as f32 + 8.0;
        let by = layout.title_bar.top_left.y as f32 + 19.0;
        let _ = ttf_text::draw_text_line(
            target,
            f,
            17.0,
            bx,
            by,
            title_txt,
            theme.colors.caption_on_accent,
        );
    } else {
        let title_style = MonoTextStyle::new(font, theme.colors.caption_on_accent);
        Text::with_baseline(
            title_txt,
            Point::new(
                layout.title_bar.top_left.x + 8,
                layout.title_bar.top_left.y + 6,
            ),
            title_style,
            Baseline::Top,
        )
        .draw(target)
        .ok();
    }

    // Title-bar close button (Bedrock: 27×31 proportions, fit inside 26px bar).
    // Place 3px from right edge (window bevel inset), vertically centered.
    {
        use uefi_ui::embedded_graphics::primitives::Line;
        const BTN_W: i32 = 22;
        const BTN_H: i32 = 20;
        let bx = layout.title_bar.top_left.x + layout.title_bar.size.width as i32 - 3 - BTN_W;
        let by = layout.title_bar.top_left.y + (layout.title_bar.size.height as i32 - BTN_H) / 2;
        let btn_rect = Rectangle::new(Point::new(bx, by), Size::new(BTN_W as u32, BTN_H as u32));
        let _ = draw_title_button(target, bevel, btn_rect, false);
        // × glyph — two diagonal lines, 2px stroke, centered in button
        let cx = bx + BTN_W / 2;
        let cy = by + BTN_H / 2;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        let _ = Line::new(Point::new(cx - 4, cy - 4), Point::new(cx + 4, cy + 4))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(target);
        let _ = Line::new(Point::new(cx + 4, cy - 4), Point::new(cx - 4, cy + 4))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(target);
    }

    // DC-42: draw_sunken already fills with face (#c6c6c6); no redundant fill needed.
    let _ = bevel.draw_sunken(target, layout.preview_rect);
    const CAPTION_H: i32 = 22;
    let img_frame = Rectangle::new(
        Point::new(
            layout.preview_inner.top_left.x,
            layout.preview_inner.top_left.y + CAPTION_H,
        ),
        Size::new(
            layout.preview_inner.size.width,
            layout.preview_inner.size.height.saturating_sub(CAPTION_H as u32),
        ),
    );
    if let Some(f) = ttf {
        let cap = "EB Garamond — fonts.google.com/specimen/EB+Garamond";
        let bx = layout.preview_inner.top_left.x as f32 + 6.0;
        let by = layout.preview_inner.top_left.y as f32 + 16.0;
        let _ = ttf_text::draw_text_line(target, f, 13.5, bx, by, cap, theme.colors.text_secondary);
    }
    if let Some((pixels, pw, ph)) = png_rgba {
        blit_rgba_contain(target, pixels, pw, ph, img_frame);
    }

    // DC-46: draw gallery bevel on gallery_rect (outer), paint content inside gallery_inner
    let _ = bevel.draw_sunken(target, layout.gallery_rect);
    paint_gallery(
        target,
        theme,
        bevel,
        font,
        gallery,
        &layout.gallery_inner,
        line_h,
        matches!(focus, Focus::Gallery) && win_stack.focused == 0,
    );
    if matches!(focus, Focus::Gallery) && win_stack.focused == 0 {
        Rectangle::new(layout.gallery_rect.top_left, layout.gallery_rect.size)
            .into_styled(PrimitiveStyle::with_stroke(theme.colors.border_focus, 1))
            .draw(target)
            .ok();
    }

    let _ = bevel.draw_sunken(target, layout.text_rect);
    let style = MonoTextStyle::new(font, theme.colors.text);
    // S15: use soft-wrapped display rows when char_w > 0.
    let cols = if char_w > 0 { layout.text_inner.size.width as usize / char_w as usize } else { 0 };
    let display_lines = textarea.wrapped_lines(cols);
    for (row, line) in display_lines
        .iter()
        .skip(textarea.scroll_top_line)
        .take(layout.visible_lines)
        .enumerate()
    {
        let y = layout.text_inner.top_left.y + row as i32 * line_h;
        let line_idx = textarea.scroll_top_line + row;
        if textarea.has_selection() {
            if let Some((c0, c1)) = textarea.selection_highlight_on_line(line_idx) {
                let hl_w = (c1 - c0) as i32 * char_w;
                if hl_w > 0 {
                    target.fill_rect_solid(
                        (layout.text_inner.top_left.x + 4 + c0 as i32 * char_w) as u32,
                        y as u32,
                        hl_w as u32,
                        line_h as u32,
                        theme.colors.selection_bg,
                    );
                }
            }
        }
        Text::with_baseline(
            line,
            Point::new(layout.text_inner.top_left.x + 4, y),
            style,
            Baseline::Top,
        )
        .draw(target)
        .ok();
    }

    if matches!(focus, Focus::Editor) && win_stack.focused == 0 {
        let (cx, cy) = caret_xy(textarea, &layout.text_inner, line_h, char_w);
        if cx >= layout.text_inner.top_left.x && cy >= layout.text_inner.top_left.y {
            for dy in 0..line_h.min(14) {
                let _ = target.fill_rect_solid(
                    cx as u32,
                    (cy + dy) as u32,
                    1,
                    1,
                    theme.colors.accent,
                );
            }
        }
    }

    // S16 / DC-38: full vertical scrollbar via draw_scrollbar_vertical.
    {
        let _ = draw_scrollbar_vertical(
            target,
            bevel,
            layout.sb_rect,
            scroll.thumb_center_ratio(),
            scroll.thumb_length_ratio(),
            false,
            false,
        );
    }
    if matches!(focus, Focus::Scrollbar) && win_stack.focused == 0 {
        Rectangle::new(layout.sb_rect.top_left, layout.sb_rect.size)
            .into_styled(PrimitiveStyle::with_stroke(theme.colors.border_focus, 1))
            .draw(target)
            .ok();
    }

    // DC-42: draw_raised_soft already fills with face; no redundant fill needed.
    let _ = bevel.draw_raised_soft(target, layout.aux_panel);
    let aux_note = MonoTextStyle::new(font, theme.colors.text);
    let _ = Text::with_baseline(
        "Aux panel — F6 cycles focus.",
        Point::new(layout.aux_panel.top_left.x + 8, layout.aux_panel.top_left.y + 10),
        aux_note,
        Baseline::Top,
    )
    .draw(target);
    let _ = Text::with_baseline(
        "Arrows nudge position.",
        Point::new(layout.aux_panel.top_left.x + 8, layout.aux_panel.top_left.y + 26),
        aux_note,
        Baseline::Top,
    )
    .draw(target);
    if win_stack.focused == 1 {
        Rectangle::new(layout.aux_panel.top_left, layout.aux_panel.size)
            .into_styled(PrimitiveStyle::with_stroke(theme.colors.border_focus, 2))
            .draw(target)
            .ok();
    }

    // Menubar + dropdown **last** (before modal) so submenus paint above preview / gallery / text / aux.
    // T-11: uses shared draw_menu_bar / draw_menu_popup from bedrock_controls.
    {
        let focused_i = if matches!(focus, Focus::Menu) {
            Some(nav.top.focused_index())
        } else {
            None
        };
        let labels: Vec<&str> = (0..layout.menu_cells.len())
            .filter_map(|i| nav.top.label(i))
            .collect();
        let _ = draw_menu_bar(
            target,
            layout.menu_strip,
            &layout.menu_cells,
            &labels,
            focused_i,
            font,
            theme.colors.surface,
            theme.colors.text,
            theme.colors.accent,
            theme.colors.caption_on_accent,
        );
    }

    if let Some((ti, si)) = nav.open {
        if let Some(Some(items)) = SUBMENUS.get(ti) {
            if let Some(cell) = layout.menu_cells.get(ti) {
                let popup = submenu_popup_rect(cell, items, char_w, line_h);
                let _ = draw_menu_popup(
                    target,
                    bevel,
                    popup,
                    items,
                    si,
                    line_h,
                    font,
                    theme.colors.canvas,
                    theme.colors.text,
                    theme.colors.selection_bg,
                    theme.colors.caption_on_accent,
                );
            }
        }
    }

    if popovers.is_modal_blocking() {
        let screen = Rectangle::new(Point::zero(), Size::new(w, h));
        let dim = Rgb888::new(0x28, 0x28, 0x28);
        let dlg = center_in_screen(Size::new(300, 140), screen);
        let s = screen;
        let d = dlg;
        let top_h = (d.top_left.y - s.top_left.y).max(0) as u32;
        if top_h > 0 {
            target.fill_rect_solid(s.top_left.x as u32, s.top_left.y as u32, s.size.width, top_h, dim);
        }
        let bot_y = d.top_left.y + d.size.height as i32;
        let bot_h = (s.top_left.y + s.size.height as i32 - bot_y).max(0) as u32;
        if bot_h > 0 {
            target.fill_rect_solid(s.top_left.x as u32, bot_y as u32, s.size.width, bot_h, dim);
        }
        let mid_y = d.top_left.y;
        let mid_h = d.size.height;
        let left_w = (d.top_left.x - s.top_left.x).max(0) as u32;
        if left_w > 0 && mid_h > 0 {
            target.fill_rect_solid(s.top_left.x as u32, mid_y as u32, left_w, mid_h, dim);
        }
        let right_x = d.top_left.x + d.size.width as i32;
        let right_w = (s.top_left.x + s.size.width as i32 - right_x).max(0) as u32;
        if right_w > 0 && mid_h > 0 {
            target.fill_rect_solid(right_x as u32, mid_y as u32, right_w, mid_h, dim);
        }

        // DC-42: draw_raised_soft already fills face; no redundant fill needed.
        let _ = bevel.draw_raised_soft(target, dlg);
        // Title bar
        const DLG_TITLE_H: i32 = 26;
        let title_rect = Rectangle::new(dlg.top_left, Size::new(dlg.size.width, DLG_TITLE_H as u32));
        let _ = draw_window_title_bar(
            target, title_rect, "About uefi_ui", font, true,
            theme.colors.accent, theme.colors.caption_on_accent, theme.colors.border,
        );
        // Title bar close button
        {
            use uefi_ui::embedded_graphics::primitives::Line;
            const BTN_W: i32 = 20;
            const BTN_H: i32 = 18;
            let bx = dlg.top_left.x + dlg.size.width as i32 - 3 - BTN_W;
            let by = dlg.top_left.y + (DLG_TITLE_H - BTN_H) / 2;
            let btn = Rectangle::new(Point::new(bx, by), Size::new(BTN_W as u32, BTN_H as u32));
            let _ = draw_title_button(target, bevel, btn, false);
            let cx = bx + BTN_W / 2;
            let cy = by + BTN_H / 2;
            let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
            let _ = Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
                .into_styled(PrimitiveStyle::with_stroke(ink, 2))
                .draw(target);
            let _ = Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
                .into_styled(PrimitiveStyle::with_stroke(ink, 2))
                .draw(target);
        }
        // Body text
        let body_style = MonoTextStyle::new(font, theme.colors.text);
        let _ = Text::with_baseline(
            "Immediate-mode widgets + UEFI.",
            Point::new(dlg.top_left.x + 16, dlg.top_left.y + DLG_TITLE_H + 10),
            body_style,
            Baseline::Top,
        )
        .draw(target);
        let _ = Text::with_baseline(
            "Press OK or Escape to close.",
            Point::new(dlg.top_left.x + 16, dlg.top_left.y + DLG_TITLE_H + 28),
            body_style,
            Baseline::Top,
        )
        .draw(target);
        // OK button — default button (activated by Enter), gets extra outer ring
        const BTN_W: u32 = 80;
        const BTN_H: u32 = 26;
        let ok_x = dlg.top_left.x + (dlg.size.width as i32 - BTN_W as i32) / 2;
        let ok_y = dlg.top_left.y + dlg.size.height as i32 - BTN_H as i32 - 10;
        let ok_rect = Rectangle::new(Point::new(ok_x, ok_y), Size::new(BTN_W, BTN_H));
        let _ = draw_button(target, bevel, ok_rect, "OK", font, theme.colors.text, false, true, true);
    }
}

/// One-shot render of the firmware demo UI for host PNG/SDL (**same layout + paint** as UEFI).
///
/// Pass an optional loaded TTF for **EB Garamond** title/caption (same as firmware); `None` uses mono.
pub fn paint_demo_snapshot(
    buf: &mut [u8],
    width: u32,
    height: u32,
    stride_bytes: usize,
    ttf: Option<&Font>,
) -> Option<()> {
    let mut fb = BgrxFramebuffer::new(buf, width, height, stride_bytes)?;
    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;
    let font = &uefi_ui::embedded_graphics::mono_font::ascii::FONT_6X10;
    let line_h = font.character_size.height as i32 + 3;
    let char_w = font.character_size.width as i32;
    let nav = NavBar::new(crate::layout::MENU_LABELS, crate::layout::SUBMENUS);
    let mut gallery = GalleryState::new();
    gallery.fs_hint = String::from("Volume: (host snapshot)");
    let textarea = TextArea::from_str(
        "Tab: menu/editor/gallery\n\
         F6: aux window\n\
         Arrows: nudge window\n\
         Firmware demo scene\n",
    );
    let focus = Focus::Editor;
    let layout = crate::layout::compute_layout(
        width as usize,
        height as usize,
        line_h,
        char_w as u32,
        WindowOffset::ZERO,
    );
    let popovers = PopoverStack::default();
    let win_stack = WindowStack::new(2);
    let png = decode_demo_png(DEMO_PNG_BYTES);
    // S15: scrollbar total reflects wrapped line count.
    let cols = if char_w > 0 { layout.text_inner.size.width as usize / char_w as usize } else { 0 };
    let total_display_rows = textarea.wrapped_line_count(cols).max(1);
    let visible = layout.visible_lines.max(1);
    let scroll = ScrollbarState::new(ScrollAxis::Vertical, total_display_rows, visible, textarea.scroll_top_line);
    paint_scene_no_cursor(
        &mut fb,
        width,
        height,
        &theme,
        &bevel,
        font,
        ttf,
        &nav,
        &gallery,
        &textarea,
        png.as_ref().map(|(b, pw, ph)| (b.as_slice(), *pw, *ph)),
        focus,
        &scroll,
        line_h,
        char_w,
        &layout,
        &popovers,
        &win_stack,
    );
    Some(())
}
