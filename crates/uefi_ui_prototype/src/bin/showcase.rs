//! Renders Bedrock UI element screenshots to `docs/screenshots/` and generates `docs/showcase.md`.
//!
//! Run: `cargo run -p uefi_ui_prototype --bin showcase`

use std::fs;

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoFont, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay};

use uefi_ui::popover::center_in_screen;
use uefi_ui::{
    bedrock::BedrockBevel,
    bedrock_controls::{
        compute_file_picker_layout, draw_checkbox_classic,
        draw_combobox_chrome, draw_dropdown_glyph, draw_file_picker,
        draw_hatched_background, draw_listbox_row,
        draw_progress_bar, draw_radio_row, draw_raised_pressed, draw_scrollbar_arrow,
        draw_scrollbar_vertical, draw_separator_h, draw_separator_v, draw_slider_track_thumb,
        draw_status_border, draw_sunken_field, draw_tab_strip, draw_title_button,
        draw_tooltip_chrome, draw_tree_view, FP_SB_W,
    },
    layout::pad,
    theme::Theme,
    widgets::{
        DirEntry, FilePickerDialogState, FilePickerFocus, LineGraph, PickerMode, ProgressBar,
        ScrollAxis, ScrollbarState, Slider, TreeNode, TreeViewState,
    },
};

/// Mock types for keyboard layout picker screenshot (keyboard_layout module requires uefi feature)
#[derive(Debug, Clone)]
struct MockKeyboardLayout {
    descriptor: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MockKeyboardLayoutPickerFocus {
    List,
    OkButton,
    CancelButton,
}

#[derive(Debug, Clone)]
struct MockKeyboardLayoutPickerState {
    layouts: Vec<MockKeyboardLayout>,
    selected: usize,
    scroll_top: usize,
    focus: MockKeyboardLayoutPickerFocus,
}

impl MockKeyboardLayoutPickerState {
    fn new(layouts: Vec<MockKeyboardLayout>) -> Self {
        Self {
            layouts,
            selected: 0,
            scroll_top: 0,
            focus: MockKeyboardLayoutPickerFocus::List,
        }
    }
    
    fn list_focused(&self) -> bool {
        matches!(self.focus, MockKeyboardLayoutPickerFocus::List)
    }
    
    fn ok_focused(&self) -> bool {
        matches!(self.focus, MockKeyboardLayoutPickerFocus::OkButton)
    }
    
    fn cancel_focused(&self) -> bool {
        matches!(self.focus, MockKeyboardLayoutPickerFocus::CancelButton)
    }
}

/// Mock layout for keyboard layout picker
#[derive(Debug, Clone, Copy)]
struct MockKeyboardLayoutPickerLayout {
    pub dialog: Rectangle,
    pub title_bar: Rectangle,
    pub close_btn: Rectangle,
    pub list_outer: Rectangle,
    pub list_inner: Rectangle,
    pub sb_rect: Rectangle,
    pub ok_btn: Rectangle,
    pub cancel_btn: Rectangle,
    pub visible_rows: usize,
}

/// Mock draw function for keyboard layout picker
fn draw_keyboard_layout_picker_mock<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    layout: &MockKeyboardLayoutPickerLayout,
    state: &MockKeyboardLayoutPickerState,
    colors: &uefi_ui::theme::ThemeColors,
    font: &MonoFont<'_>,
    line_h: i32,
) -> Result<(), D::Error> {
    use embedded_graphics::prelude::*;
    use embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
    use embedded_graphics::text::{Baseline, Text};

    let font_h = font.character_size.height as i32;

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
    {
        let cx = layout.close_btn.top_left.x + layout.close_btn.size.width as i32 / 2;
        let cy = layout.close_btn.top_left.y + layout.close_btn.size.height as i32 / 2;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
        Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2)).draw(target)?;
    }

    // Layout list
    if state.list_focused() {
        Rectangle::new(layout.list_outer.top_left, layout.list_outer.size)
            .into_styled(PrimitiveStyle::with_stroke(colors.text, 1))
            .draw(target)?;
    }
    uefi_ui::bedrock_controls::draw_sunken_field(target, bevel, layout.list_outer, colors.canvas)?;

    // Draw list items
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

    // Scrollbar
    uefi_ui::bedrock_controls::draw_scrollbar_vertical(
        target, bevel, layout.sb_rect,
        Some(0.5),  // mock thumb center
        Some(0.3),  // mock thumb length
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

/// Mock layout computation for keyboard layout picker
fn compute_keyboard_layout_picker_layout_mock(dialog: Rectangle, line_h: i32) -> MockKeyboardLayoutPickerLayout {
    let x = dialog.top_left.x;
    let y = dialog.top_left.y;
    let dw = dialog.size.width;
    let dh = dialog.size.height;
    let bevel_px = BedrockBevel::BEVEL_PX as i32; // 3
    let inner_pad = bevel_px + 5; // 8px inner margin

    let title_bar = Rectangle::new(Point::new(x, y), Size::new(dw, 26));
    // Close button: 22x20
    let close_btn = Rectangle::new(
        Point::new(x + dw as i32 - 3 - 22, y + (26 as i32 - 20) / 2),
        Size::new(22, 20),
    );

    // List area
    let list_top = y + 26 + inner_pad;
    let btn_y = y + dh as i32 - 38 + (38 - 22) / 2;
    let list_h = (btn_y - list_top).max(0) as u32;
    let list_outer = Rectangle::new(
        Point::new(x + inner_pad, list_top),
        Size::new(dw - inner_pad as u32 * 2, list_h),
    );
    let list_inner = pad(list_outer, BedrockBevel::BEVEL_PX);
    let sb_rect = Rectangle::new(
        Point::new(
            list_outer.top_left.x + list_outer.size.width as i32 - 26 as i32,
            list_outer.top_left.y,
        ),
        Size::new(26, list_h),
    );

    // Buttons - right-aligned
    let cancel_btn = Rectangle::new(
        Point::new(x + dw as i32 - inner_pad - 80, btn_y),
        Size::new(80, 22),
    );
    let ok_btn = Rectangle::new(
        Point::new(cancel_btn.top_left.x - 80 - 8, btn_y),
        Size::new(80, 22),
    );

    let visible_rows = (list_h as i32 / line_h.max(1)) as usize;

    MockKeyboardLayoutPickerLayout {
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

// ─── helpers ────────────────────────────────────────────────────────────────

type Disp = SimulatorDisplay<Rgb888>;

fn canvas(w: u32, h: u32, bg: Rgb888) -> Disp {
    let mut d = Disp::new(Size::new(w, h));
    rect(&mut d, Rectangle::new(Point::zero(), Size::new(w, h)), bg);
    d
}

fn rect(d: &mut Disp, r: Rectangle, c: Rgb888) {
    r.into_styled(PrimitiveStyleBuilder::new().fill_color(c).build())
        .draw(d)
        .ok();
}

fn label(d: &mut Disp, x: i32, y: i32, s: &str, c: Rgb888) {
    let style = MonoTextStyle::new(&FONT_6X10, c);
    Text::with_baseline(s, Point::new(x, y), style, Baseline::Top)
        .draw(d)
        .ok();
}

fn save(d: &Disp, path: &str) {
    let settings = OutputSettingsBuilder::new().scale(2).build();
    let img = d.to_rgb_output_image(&settings);
    img.save_png(path).expect(path);
    eprintln!("  wrote {path}");
}

// ─── per-element renderers ───────────────────────────────────────────────────

fn render_bevel_styles(out: &str, bevel: &BedrockBevel, bg: Rgb888, ink: Rgb888) {
    let mut d = canvas(570, 140, bg);
    let labels = ["raised", "window", "sunken", "groupbox", "status"];
    let mut x = 10;
    for (i, lbl) in labels.iter().enumerate() {
        let r = Rectangle::new(Point::new(x, 24), Size::new(96, 80));
        match i {
            0 => bevel.draw_raised(&mut d, r).ok(),
            1 => bevel.draw_raised_soft(&mut d, r).ok(),
            2 => bevel.draw_sunken(&mut d, r).ok(),
            3 => bevel
                .draw_groupbox(&mut d, r, Some((x + 10, 36)), Some(bg))
                .ok(),
            _ => bevel.draw_status_border(&mut d, r).ok(),
        };
        label(&mut d, x + 6, 114, lbl, ink);
        x += 112;
    }
    save(&d, out);
}

fn render_buttons(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(360, 120, theme.colors.surface);
    let items: &[(&str, bool, bool)] = &[
        ("Normal", false, false),
        ("Pressed", false, true),
        ("Title ×", true, false),
    ];
    let mut x = 14;
    for (lbl, title, pressed) in items {
        let r = Rectangle::new(Point::new(x, 24), Size::new(96, 28));
        if *title {
            draw_title_button(&mut d, bevel, r, *pressed).ok();
            // Draw × glyph
            let cx = r.top_left.x + r.size.width as i32 / 2;
            let cy = r.top_left.y + r.size.height as i32 / 2;
            let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
            Line::new(Point::new(cx - 4, cy - 4), Point::new(cx + 4, cy + 4))
                .into_styled(PrimitiveStyle::with_stroke(ink, 2))
                .draw(&mut d)
                .ok();
            Line::new(Point::new(cx + 4, cy - 4), Point::new(cx - 4, cy + 4))
                .into_styled(PrimitiveStyle::with_stroke(ink, 2))
                .draw(&mut d)
                .ok();
        } else if *pressed {
            draw_raised_pressed(&mut d, bevel, r).ok();
        } else {
            bevel.draw_raised(&mut d, r).ok();
        }
        let off = if *pressed { 1 } else { 0 };
        label(
            &mut d,
            r.top_left.x + 10 + off,
            r.top_left.y + 9 + off,
            lbl,
            theme.colors.text,
        );
        label(&mut d, x + 6, 72, lbl, theme.colors.text_secondary);
        x += 114;
    }
    save(&d, out);
}

fn render_checkbox(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(300, 100, theme.colors.surface);
    let paper = Rgb888::WHITE;
    let r0 = Rectangle::new(Point::new(80, 28), Size::new(20, 20));
    let r1 = Rectangle::new(Point::new(180, 28), Size::new(20, 20));
    draw_checkbox_classic(&mut d, bevel, r0, false, paper, true).ok();
    draw_checkbox_classic(&mut d, bevel, r1, true, paper, true).ok();
    label(&mut d, 14, 33, "Unchecked", theme.colors.text);
    label(&mut d, 110, 33, "Checked", theme.colors.text);
    label(
        &mut d,
        14,
        70,
        "Checkbox (20×20)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_radio(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(340, 100, theme.colors.surface);
    let row = Rectangle::new(Point::new(14, 28), Size::new(3 * 20 + 2 * 8, 20));
    draw_radio_row(&mut d, row, 3, 1, bevel, true).ok();
    label(
        &mut d,
        14,
        72,
        "RadioGroup (3 items, index 1 selected)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_toggle(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(280, 100, theme.colors.surface);
    let paper = Rgb888::WHITE;
    let off_r = Rectangle::new(Point::new(14, 28), Size::new(52, 22));
    let on_r = Rectangle::new(Point::new(100, 28), Size::new(52, 22));
    draw_sunken_field(&mut d, bevel, off_r, paper).ok();
    label(
        &mut d,
        off_r.top_left.x + 6,
        off_r.top_left.y + 6,
        "OFF",
        theme.colors.text,
    );
    draw_sunken_field(&mut d, bevel, on_r, theme.colors.selection_bg).ok();
    label(
        &mut d,
        on_r.top_left.x + 6,
        on_r.top_left.y + 6,
        "ON",
        theme.colors.caption_on_accent,
    );
    label(
        &mut d,
        14,
        74,
        "Toggle (OFF / ON)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_slider(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(320, 100, theme.colors.surface);
    let slider = Slider::new(0.0, 100.0, 33.0);
    let track = Rectangle::new(Point::new(14, 36), Size::new(280, 14));
    draw_slider_track_thumb(&mut d, bevel, track, slider.ratio(), 10).ok();
    label(&mut d, 14, 74, "Slider at 33%", theme.colors.text_secondary);
    save(&d, out);
}

fn render_progress(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(320, 100, theme.colors.surface);
    let pb = ProgressBar::new(0.65);
    let track = Rectangle::new(Point::new(14, 36), Size::new(280, 16));
    draw_progress_bar(
        &mut d,
        bevel,
        track,
        pb.value,
        theme.colors.progress_track,
        theme.colors.progress_fill,
    )
    .ok();
    label(
        &mut d,
        14,
        78,
        "ProgressBar at 65%",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_tabs(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(420, 120, theme.colors.surface);
    let tl = Point::new(14, 28);
    let widths = [80u32, 80, 100];
    let sel = 1usize;
    draw_tab_strip(&mut d, bevel, tl, 36, &widths, sel, theme.colors.surface).ok();
    let tab_labels = ["Settings", "Advanced", "About"];
    let mut tx = tl.x;
    for (i, name) in tab_labels.iter().enumerate() {
        let ty = if i == sel { tl.y - 4 + 13 } else { tl.y + 13 };
        label(&mut d, tx + 10, ty, name, theme.colors.text);
        tx += widths[i] as i32 + 2;
    }
    label(
        &mut d,
        14,
        90,
        "Tabs (36px inactive / 40px active)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_combobox(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(320, 100, theme.colors.surface);
    let paper = Rgb888::WHITE;
    let field = Rectangle::new(Point::new(14, 30), Size::new(210, 22));
    let btn = Rectangle::new(
        Point::new(field.top_left.x + field.size.width as i32, 30),
        Size::new(30, 22),
    );
    draw_combobox_chrome(&mut d, bevel, field, btn, paper).ok();
    let dfill = pad(field, BedrockBevel::BEVEL_PX);
    label(
        &mut d,
        dfill.top_left.x + 2,
        dfill.top_left.y + 3,
        "Green",
        theme.colors.text,
    );
    draw_dropdown_glyph(&mut d, btn, theme.colors.text).ok();
    label(
        &mut d,
        14,
        78,
        "ComboBox (closed state)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_listbox(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let paper = Rgb888::WHITE;
    let items = ["Alpha", "Beta", "Gamma"];
    let lh = 22i32;
    let lbox = Rectangle::new(
        Point::new(14, 14),
        Size::new(240, (items.len() as i32 * lh + 8) as u32),
    );
    let mut d = canvas(
        lbox.size.width + 28,
        lbox.size.height + 40,
        theme.colors.surface,
    );
    draw_sunken_field(&mut d, bevel, lbox, paper).ok();
    let lfill = pad(lbox, 2);
    let font_h = 10i32; // FONT_6X10 height
    for (j, name) in items.iter().enumerate() {
        let row_y = lfill.top_left.y + 4 + j as i32 * lh;
        draw_listbox_row(
            &mut d,
            lfill,
            row_y,
            lh,
            j == 0,
            false,
            paper,
            theme.colors.selection_bg,
        )
        .ok();
        label(
            &mut d,
            lfill.top_left.x + 4,
            row_y + (lh - font_h) / 2,
            name,
            if j == 0 {
                theme.colors.caption_on_accent
            } else {
                theme.colors.text
            },
        );
    }
    label(
        &mut d,
        14,
        lbox.top_left.y + lbox.size.height as i32 + 6,
        "ListBox (first item selected)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_scrollbar(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let sb_w = 26u32;
    let sb_h = 200u32;
    let mut d = canvas(sb_w + 16, sb_h + 16, theme.colors.surface);
    let sb = Rectangle::new(Point::new(8, 8), Size::new(sb_w, sb_h));
    let arrow_h = sb_w;
    let track = Rectangle::new(
        Point::new(sb.top_left.x, sb.top_left.y + arrow_h as i32),
        Size::new(sb_w, sb_h.saturating_sub(arrow_h * 2)),
    );
    draw_hatched_background(&mut d, track, bevel.face, bevel.border_lightest).ok();
    let up = Rectangle::new(sb.top_left, Size::new(sb_w, arrow_h));
    let dn = Rectangle::new(
        Point::new(sb.top_left.x, sb.top_left.y + sb_h as i32 - arrow_h as i32),
        Size::new(sb_w, arrow_h),
    );
    draw_scrollbar_arrow(&mut d, bevel, up, 0, false).ok();
    draw_scrollbar_arrow(&mut d, bevel, dn, 1, false).ok();
    // Draw thumb at ~40%
    let thumb_h = 40u32;
    let avail = track.size.height.saturating_sub(thumb_h);
    let thumb = Rectangle::new(
        Point::new(
            track.top_left.x,
            track.top_left.y + (avail as f32 * 0.4) as i32,
        ),
        Size::new(sb_w, thumb_h),
    );
    bevel.draw_raised(&mut d, thumb).ok();
    save(&d, out);
}

fn render_separators(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(340, 120, theme.colors.surface);
    draw_separator_h(&mut d, bevel, 14, 160, 38).ok();
    draw_separator_h(&mut d, bevel, 14, 160, 58).ok();
    draw_separator_v(&mut d, bevel, 220, 22, 80).ok();
    draw_separator_v(&mut d, bevel, 238, 22, 80).ok();
    label(
        &mut d,
        14,
        80,
        "Horizontal (2px etched)",
        theme.colors.text_secondary,
    );
    label(&mut d, 210, 94, "V", theme.colors.text_secondary);
    save(&d, out);
}

fn render_groupbox(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(360, 150, theme.colors.surface);
    let r = Rectangle::new(Point::new(14, 24), Size::new(320, 100));
    let lbl = "Connection";
    let gap_x = r.top_left.x + 10;
    let gap_w = lbl.len() as i32 * 6 + 4;
    bevel
        .draw_groupbox(&mut d, r, Some((gap_x, gap_w)), Some(theme.colors.surface))
        .ok();
    label(&mut d, gap_x + 2, r.top_left.y - 5, lbl, theme.colors.text);
    label(
        &mut d,
        28,
        68,
        "Label text inside the etched groupbox border.",
        theme.colors.text,
    );
    save(&d, out);
}

fn render_tooltip(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(280, 90, theme.colors.surface);
    let tip = Rectangle::new(Point::new(30, 24), Size::new(210, 30));
    draw_tooltip_chrome(&mut d, bevel, tip, theme.colors.tooltip_bg).ok();
    label(
        &mut d,
        tip.top_left.x + 8,
        tip.top_left.y + 10,
        "Tooltip: hover info here",
        theme.colors.text,
    );
    save(&d, out);
}

fn render_hatched(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(260, 100, theme.colors.surface);
    let area = Rectangle::new(Point::new(14, 20), Size::new(224, 44));
    bevel.draw_sunken(&mut d, area).ok();
    let inner = pad(area, BedrockBevel::BEVEL_PX);
    draw_hatched_background(&mut d, inner, bevel.face, bevel.border_lightest).ok();
    label(
        &mut d,
        14,
        82,
        "Hatched track (scrollbar / pressed)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_status_bar(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(360, 80, theme.colors.surface);
    let bar = Rectangle::new(Point::new(0, 28), Size::new(360, 26));
    rect(&mut d, bar, theme.colors.surface);
    draw_status_border(&mut d, bevel, bar).ok();
    label(
        &mut d,
        10,
        bar.top_left.y + 8,
        "Ready  |  Objects: 3  |  Modified",
        theme.colors.text,
    );
    save(&d, out);
}

fn render_graph(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let mut d = canvas(320, 100, theme.colors.surface);
    let area = Rectangle::new(Point::new(14, 14), Size::new(280, 54));
    bevel.draw_sunken(&mut d, area).ok();
    let inner = pad(area, 2);
    let mut g = LineGraph::new(48);
    for i in 0..48usize {
        g.push((i as f32 * 0.2).sin() * 13.0 + 20.0);
    }
    for w in g.points(inner).windows(2) {
        Line::new(w[0], w[1])
            .into_styled(PrimitiveStyle::with_stroke(theme.colors.graph_line, 1))
            .draw(&mut d)
            .ok();
    }
    label(
        &mut d,
        14,
        82,
        "LineGraph (sine wave)",
        theme.colors.text_secondary,
    );
    save(&d, out);
}

fn render_file_picker(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let line_h = FONT_6X10.character_size.height as i32 + 8;
    let char_w = FONT_6X10.character_size.width as i32;

    let dw = 480u32;
    let dh = 320u32;
    let mut d = canvas(dw + 30, dh + 30, theme.colors.background);
    let dialog = Rectangle::new(Point::new(15, 15), Size::new(dw, dh));
    let layout = compute_file_picker_layout(dialog, line_h, FP_SB_W);

    // Build state via FilePickerDialogState with mock filesystem
    struct MockFs;
    impl uefi_ui::file_picker::FileIo for MockFs {
        type Error = ();
        fn list(&mut self, _: &[String]) -> Result<Vec<DirEntry>, ()> {
            Ok(vec![
                DirEntry {
                    name: String::from("drivers"),
                    is_dir: true,
                },
                DirEntry {
                    name: String::from("tools"),
                    is_dir: true,
                },
                DirEntry {
                    name: String::from("BOOTX64.EFI"),
                    is_dir: false,
                },
                DirEntry {
                    name: String::from("Shell.efi"),
                    is_dir: false,
                },
                DirEntry {
                    name: String::from("startup.nsh"),
                    is_dir: false,
                },
            ])
        }
        fn read_file(&mut self, _: &[String], _: &str) -> Result<Vec<u8>, ()> {
            Ok(vec![])
        }
        fn write_file(&mut self, _: &[String], _: &str, _: &[u8]) -> Result<(), ()> {
            Ok(())
        }
    }

    let mut dlg = FilePickerDialogState::new(PickerMode::Load, 3, &mut MockFs).unwrap();
    dlg.picker.path = vec![String::from("EFI"), String::from("Boot")];
    dlg.picker.selected = 2; // BOOTX64.EFI
    dlg.filename = uefi_ui::file_picker::LineInput::from_str("BOOTX64.EFI");
    // Show with focus on list
    dlg.focus = FilePickerFocus::List;

    let scroll = ScrollbarState::new(
        ScrollAxis::Vertical,
        dlg.picker.entries.len().max(1),
        layout.visible_rows.max(1),
        dlg.picker.scroll_top,
    );
    let filetype_labels = &["All files (*.*)", "EFI binaries (*.efi)", "Scripts (*.nsh)"];

    draw_file_picker(
        &mut d,
        bevel,
        &layout,
        &dlg.picker,
        &dlg.filename.text,
        filetype_labels,
        dlg.filetype_sel,
        true,
        dlg.focus,
        line_h,
        char_w,
        &FONT_6X10,
        &theme.colors,
        &scroll,
    )
    .ok();

    save(&d, out);
}

fn render_tree_view(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    let line_h = FONT_6X10.character_size.height as i32 + 8;

    // 320×260 canvas: left tree pane (180) + right file list placeholder (120), 20px margins
    let mut d = canvas(360, 280, theme.colors.background);

    // Build a sample tree: C: drive with EFI subtree expanded
    let mut root = TreeNode::new("C:", "C:").with_children(vec![
        TreeNode::new("EFI", "EFI").with_children(vec![
            TreeNode::new("Boot", "Boot"),
            TreeNode::new("Microsoft", "Microsoft")
                .with_children(vec![TreeNode::new("Recovery", "Recovery")]),
        ]),
        TreeNode::new("Windows", "Windows"),
        TreeNode::new("Users", "Users"),
    ]);
    root.expanded = true;

    // Expand EFI subtree
    if let Some(efi) = root.children.iter_mut().find(|n| n.path_component == "EFI") {
        efi.expanded = true;
    }

    let mut tree = TreeViewState::new(vec![root]);
    tree.selected_path = vec![
        String::from("C:"),
        String::from("EFI"),
        String::from("Boot"),
    ];

    // Left pane with scrollbar
    let sb_w = 26u32;
    let pane = Rectangle::new(Point::new(10, 10), Size::new(200, 260));
    let tree_rect = Rectangle::new(
        pane.top_left,
        Size::new(pane.size.width - sb_w, pane.size.height),
    );
    let sb_rect = Rectangle::new(
        Point::new(
            pane.top_left.x + tree_rect.size.width as i32,
            pane.top_left.y,
        ),
        Size::new(sb_w, pane.size.height),
    );

    draw_tree_view(
        &mut d,
        bevel,
        tree_rect,
        &tree,
        line_h,
        &FONT_6X10,
        &theme.colors,
    )
    .ok();

    let total = tree.flat_row_count().max(1);
    let visible = (tree_rect.size.height as usize / line_h as usize).max(1);
    let sb = ScrollbarState::new(ScrollAxis::Vertical, total, visible, tree.scroll_top);
    use uefi_ui::bedrock_controls::draw_scrollbar_vertical;
    draw_scrollbar_vertical(
        &mut d,
        bevel,
        sb_rect,
        sb.thumb_center_ratio(),
        sb.thumb_length_ratio(),
        false,
        false,
    )
    .ok();

    // Right pane label (placeholder)
    use uefi_ui::bedrock_controls::draw_sunken_field;
    let right_pane = Rectangle::new(Point::new(220, 10), Size::new(130, 260));
    draw_sunken_field(&mut d, bevel, right_pane, theme.colors.canvas).ok();
    Text::with_baseline(
        "File list",
        Point::new(right_pane.top_left.x + 8, right_pane.top_left.y + 8),
        MonoTextStyle::new(&FONT_6X10, theme.colors.text),
        Baseline::Top,
    )
    .draw(&mut d)
    .ok();

    // A separator between the two panes
    use uefi_ui::bedrock_controls::draw_separator_v;
    draw_separator_v(&mut d, bevel, 215, 10, 269).ok();

    save(&d, out);
}

/// Render the keyboard layout picker dialog with mock data.
fn render_keyboard_layout_picker(out: &str, theme: &Theme, bevel: &BedrockBevel) {
    // Create sample keyboard layouts
    let layouts = vec![
        MockKeyboardLayout { descriptor: String::from("US English") },
        MockKeyboardLayout { descriptor: String::from("Danish") },
        MockKeyboardLayout { descriptor: String::from("German") },
        MockKeyboardLayout { descriptor: String::from("French") },
        MockKeyboardLayout { descriptor: String::from("Spanish") },
    ];

    // Dialog dimensions matching the real one
    let dw = 400u32;
    let dh = 230u32;
    let line_h = FONT_6X10.character_size.height as i32 + 6;
    
    let mut d = canvas(dw, dh, theme.colors.background);
    let screen_rect = Rectangle::new(Point::new(0, 0), Size::new(dw, dh));
    let dlg = center_in_screen(Size::new(dw, dh), screen_rect);
    
    let layout = compute_keyboard_layout_picker_layout_mock(dlg, line_h);
    let state = MockKeyboardLayoutPickerState::new(layouts);

    draw_keyboard_layout_picker_mock(
        &mut d,
        bevel,
        &layout,
        &state,
        &theme.colors,
        &FONT_6X10,
        line_h,
    )
    .ok();

    save(&d, out);
}

// (draw_groupbox_border and draw_status_border are now in uefi_ui::bedrock_controls)

// ─── editor screenshots ───────────────────────────────────────────────────────

const TINOS_PATH: &str = "assets/fonts/Tinos-Regular.ttf";

const ED_W: u32 = 900;
const ED_H: u32 = 660;
const ED_TITLE_H: i32 = 24;
const ED_MENU_H: i32 = 22;
const ED_SB_W: i32 = 17;
const ED_STATUS_H: i32 = 22;
const ED_FINDBAR_H: i32 = 28;
const ED_PAD_X: i32 = 6;
const ED_PAD_TOP: i32 = 4;

fn ed_blend(bg: Rgb888, fg: Rgb888, a: u8) -> Rgb888 {
    let b = a as u16;
    let ib = 255 - b;
    Rgb888::new(
        ((fg.r() as u16 * b + bg.r() as u16 * ib) / 255) as u8,
        ((fg.g() as u16 * b + bg.g() as u16 * ib) / 255) as u8,
        ((fg.b() as u16 * b + bg.b() as u16 * ib) / 255) as u8,
    )
}

/// Draw a text string using fontdue glyphs onto a SimulatorDisplay.
fn draw_font_text(
    d: &mut Disp,
    font: &fontdue::Font,
    text: &str,
    px: f32,
    x0: i32,
    baseline_y: i32,
    fg: Rgb888,
    bg: Rgb888,
    clip: Rectangle,
) -> i32 {
    let dw = d.size().width as i32;
    let dh = d.size().height as i32;
    let mut x = x0;
    for ch in text.chars() {
        if x >= clip.top_left.x + clip.size.width as i32 {
            break;
        }
        let (m, bmp) = font.rasterize(ch, px);
        let gx = x;
        let gy = baseline_y - (m.ymin + m.height as i32);
        for row in 0..m.height {
            for col in 0..m.width {
                let alpha = bmp[row * m.width + col];
                if alpha == 0 {
                    continue;
                }
                let px_x = gx + col as i32;
                let px_y = gy + row as i32;
                if px_x < clip.top_left.x || px_x >= clip.top_left.x + clip.size.width as i32 {
                    continue;
                }
                if px_y < clip.top_left.y || px_y >= clip.top_left.y + clip.size.height as i32 {
                    continue;
                }
                if px_x < 0 || px_x >= dw || px_y < 0 || px_y >= dh {
                    continue;
                }
                Pixel(Point::new(px_x, px_y), ed_blend(bg, fg, alpha))
                    .draw(d)
                    .ok();
            }
        }
        x += m.advance_width.round() as i32;
    }
    x
}

fn ed_line_height(px: f32) -> i32 {
    (px * 1.4).ceil() as i32
}

fn ed_fill(d: &mut Disp, r: Rectangle, c: Rgb888) {
    r.into_styled(PrimitiveStyleBuilder::new().fill_color(c).build())
        .draw(d)
        .ok();
}

fn ed_draw_title(d: &mut Disp, bevel: &BedrockBevel, theme: &Theme, title: &str) {
    let r = Rectangle::new(Point::zero(), Size::new(ED_W, ED_TITLE_H as u32));
    ed_fill(d, r, theme.colors.accent);
    let style = MonoTextStyle::new(&FONT_6X10, theme.colors.caption_on_accent);
    Text::with_baseline(
        title,
        Point::new(8, (ED_TITLE_H - 10) / 2),
        style,
        Baseline::Top,
    )
    .draw(d)
    .ok();
    // Close button
    let cb_w = 20i32;
    let cb_h = 18i32;
    let cb = Rectangle::new(
        Point::new(ED_W as i32 - 3 - cb_w, (ED_TITLE_H - cb_h) / 2),
        Size::new(cb_w as u32, cb_h as u32),
    );
    draw_title_button(d, bevel, cb, false).ok();
    let cx = cb.top_left.x + cb.size.width as i32 / 2;
    let cy = cb.top_left.y + cb.size.height as i32 / 2;
    let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
    Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
        .into_styled(PrimitiveStyle::with_stroke(ink, 2))
        .draw(d)
        .ok();
    Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
        .into_styled(PrimitiveStyle::with_stroke(ink, 2))
        .draw(d)
        .ok();
}

fn ed_draw_menubar(d: &mut Disp, theme: &Theme, open_menu: Option<usize>) {
    let bar = Rectangle::new(Point::new(0, ED_TITLE_H), Size::new(ED_W, ED_MENU_H as u32));
    ed_fill(d, bar, theme.colors.surface);
    let items = &["File", "Edit"];
    let cell_w = 54i32;
    let mut cx = 4i32;
    for (i, lbl) in items.iter().enumerate() {
        let sel = open_menu == Some(i);
        let bg = if sel {
            theme.colors.accent
        } else {
            theme.colors.surface
        };
        let fg = if sel {
            theme.colors.caption_on_accent
        } else {
            theme.colors.text
        };
        let cell = Rectangle::new(
            Point::new(cx, ED_TITLE_H),
            Size::new(cell_w as u32, ED_MENU_H as u32),
        );
        ed_fill(d, cell, bg);
        let style = MonoTextStyle::new(&FONT_6X10, fg);
        Text::with_baseline(
            lbl,
            Point::new(cx + 6, ED_TITLE_H + (ED_MENU_H - 10) / 2),
            style,
            Baseline::Top,
        )
        .draw(d)
        .ok();
        cx += cell_w + 2;
    }
}

fn ed_draw_file_menu_popup(d: &mut Disp, bevel: &BedrockBevel, theme: &Theme, sel: usize) {
    use uefi_ui::bedrock_controls::draw_menu_popup_ex;
    let items = &[
        "New\tCtrl+N",
        "Open Last File",
        "Open...\tCtrl+O",
        "Save\tCtrl+S",
        "Save As...",
        "—",
        "Quit\tq",
    ];
    // "Open Last File" is greyed (index 1) — no last file in this screenshot
    let disabled = &[false, true, false, false, false, false, false];
    let popup = Rectangle::new(Point::new(4, ED_TITLE_H + ED_MENU_H), Size::new(200, 160));
    draw_menu_popup_ex(
        d,
        bevel,
        popup,
        items,
        disabled,
        sel,
        22,
        &FONT_6X10,
        theme.colors.canvas,
        theme.colors.text,
        theme.colors.selection_bg,
        theme.colors.caption_on_accent,
    )
    .ok();
}

fn ed_draw_textarea_chrome(
    d: &mut Disp,
    bevel: &BedrockBevel,
    _theme: &Theme,
    ta_top: i32,
    ta_h: i32,
) {
    let ta = Rectangle::new(
        Point::new(0, ta_top),
        Size::new(ED_W - ED_SB_W as u32, ta_h as u32),
    );
    ed_fill(d, ta, Rgb888::WHITE);
    draw_sunken_field(d, bevel, ta, Rgb888::WHITE).ok();
    // Scrollbar
    let sb = Rectangle::new(
        Point::new(ED_W as i32 - ED_SB_W, ta_top),
        Size::new(ED_SB_W as u32, ta_h as u32),
    );
    let scroll = ScrollbarState::new(ScrollAxis::Vertical, 40, 28, 0);
    draw_scrollbar_vertical(
        d,
        bevel,
        sb,
        scroll.thumb_center_ratio(),
        scroll.thumb_length_ratio(),
        false,
        false,
    )
    .ok();
    // Use bedrock_controls directly
    let _ = (ta, sb);
}

fn ed_draw_status(d: &mut Disp, bevel: &BedrockBevel, theme: &Theme, text: &str) {
    let r = Rectangle::new(
        Point::new(0, ED_H as i32 - ED_STATUS_H),
        Size::new(ED_W, ED_STATUS_H as u32),
    );
    ed_fill(d, r, theme.colors.surface);
    draw_status_border(d, bevel, r).ok();
    let style = MonoTextStyle::new(&FONT_6X10, theme.colors.text);
    Text::with_baseline(
        text,
        Point::new(8, r.top_left.y + (ED_STATUS_H - 10) / 2),
        style,
        Baseline::Top,
    )
    .draw(d)
    .ok();
}

fn ed_draw_findbar(
    d: &mut Disp,
    bevel: &BedrockBevel,
    theme: &Theme,
    query: &str,
    result: &str,
    ta_bottom: i32,
) {
    let r = Rectangle::new(
        Point::new(0, ta_bottom),
        Size::new(ED_W, ED_FINDBAR_H as u32),
    );
    ed_fill(d, r, theme.colors.surface);
    draw_status_border(d, bevel, r).ok();
    let style = MonoTextStyle::new(&FONT_6X10, theme.colors.text);
    let ly = r.top_left.y + (ED_FINDBAR_H - 10) / 2;
    Text::with_baseline("Find:", Point::new(6, ly), style, Baseline::Top)
        .draw(d)
        .ok();
    let field = Rectangle::new(
        Point::new(44, r.top_left.y + 4),
        Size::new(240, (ED_FINDBAR_H - 8) as u32),
    );
    draw_sunken_field(d, bevel, field, theme.colors.canvas).ok();
    let fi = pad(field, BedrockBevel::BEVEL_PX);
    let ty = fi.top_left.y + (fi.size.height as i32 - 10) / 2;
    Text::with_baseline(
        query,
        Point::new(fi.top_left.x + 2, ty),
        style,
        Baseline::Top,
    )
    .draw(d)
    .ok();
    Text::with_baseline(result, Point::new(300, ly), style, Baseline::Top)
        .draw(d)
        .ok();
}

const SAMPLE_TEXT: &str = "\
The quick brown fox jumps over the lazy dog.
Pack my box with five dozen liquor jugs.

fn greet(name: &str) -> String {
    format!(\"Hello, {}!\", name)
}

fn main() {
    let msg = greet(\"world\");
    println!(\"{}\", msg);
    // TODO: add error handling
}

// This file was opened with the uefi_ui text editor.
// Font: Tinos Regular at 14px
// Supports: cut, copy, paste, find, save, open
";

fn render_editor_screenshot(
    font: &fontdue::Font,
    font_px: f32,
    title: &str,
    text: &str,
    open_menu: Option<usize>,
    show_find: bool,
    find_query: &str,
    find_result: &str,
    sel_line: Option<usize>, // line to highlight as selected
    theme: &Theme,
    bevel: &BedrockBevel,
    out: &str,
) {
    let mut d = canvas(ED_W, ED_H, theme.colors.surface);
    let lh = ed_line_height(font_px);

    let ta_top = ED_TITLE_H + ED_MENU_H;
    let ta_bottom_max = ED_H as i32 - ED_STATUS_H;
    let ta_h = if show_find {
        ta_bottom_max - ED_FINDBAR_H - ta_top
    } else {
        ta_bottom_max - ta_top
    };
    let ta_h = ta_h.max(0);

    // White textarea background + sunken chrome
    ed_draw_textarea_chrome(&mut d, bevel, theme, ta_top, ta_h);

    // Text content
    let inner_x = BedrockBevel::BEVEL_PX as i32; // inset from textarea sunken border
    let inner_w = ED_W as i32 - ED_SB_W - BedrockBevel::BEVEL_PX as i32 * 2;
    let clip = Rectangle::new(
        Point::new(inner_x, ta_top + BedrockBevel::BEVEL_PX as i32),
        Size::new(
            inner_w as u32,
            (ta_h - BedrockBevel::BEVEL_PX as i32 * 2).max(0) as u32,
        ),
    );

    let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate().take((ta_h / lh) as usize + 1) {
        let row_top = ta_top + BedrockBevel::BEVEL_PX as i32 + ED_PAD_TOP + i as i32 * lh;
        if row_top + lh < ta_top || row_top > ta_top + ta_h {
            continue;
        }

        // Selection highlight
        let sel_bg = Rgb888::new(0xc0, 0xc0, 0xc0);
        if sel_line == Some(i) {
            let sel_rect = Rectangle::new(
                Point::new(clip.top_left.x, row_top),
                Size::new(clip.size.width, lh as u32),
            );
            ed_fill(&mut d, sel_rect, sel_bg);
            let baseline = row_top + (lh as f32 * 0.78) as i32;
            draw_font_text(
                &mut d,
                font,
                line,
                font_px,
                clip.top_left.x + ED_PAD_X,
                baseline,
                ink,
                sel_bg,
                clip,
            );
        } else {
            let baseline = row_top + (lh as f32 * 0.78) as i32;
            draw_font_text(
                &mut d,
                font,
                line,
                font_px,
                clip.top_left.x + ED_PAD_X,
                baseline,
                ink,
                Rgb888::WHITE,
                clip,
            );
        }
    }

    // Chrome layers on top of text
    ed_draw_title(&mut d, bevel, theme, title);
    ed_draw_menubar(&mut d, theme, open_menu);

    if open_menu == Some(0) {
        ed_draw_file_menu_popup(&mut d, bevel, theme, 1);
    }

    if show_find {
        let find_top = ta_top + ta_h;
        ed_draw_findbar(&mut d, bevel, theme, find_query, find_result, find_top);
    }

    ed_draw_status(
        &mut d,
        bevel,
        theme,
        "Ln 6, Col 1     sample.txt     Font: 14px",
    );

    save(&d, out);
}

fn render_editor_with_filepicker(
    font: &fontdue::Font,
    theme: &Theme,
    bevel: &BedrockBevel,
    out: &str,
) {
    let font_px = 14.0f32;
    let lh = ed_line_height(font_px);

    let mut d = canvas(ED_W, ED_H, theme.colors.surface);
    let ta_top = ED_TITLE_H + ED_MENU_H;
    let ta_h = ED_H as i32 - ED_STATUS_H - ta_top;

    // Render dimmed background editor
    ed_draw_textarea_chrome(&mut d, bevel, theme, ta_top, ta_h);
    let clip = Rectangle::new(
        Point::new(
            BedrockBevel::BEVEL_PX as i32,
            ta_top + BedrockBevel::BEVEL_PX as i32,
        ),
        Size::new(
            ED_W - ED_SB_W as u32 - BedrockBevel::BEVEL_PX as u32 * 2,
            (ta_h - BedrockBevel::BEVEL_PX as i32 * 2).max(0) as u32,
        ),
    );
    let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
    for (i, line) in SAMPLE_TEXT
        .lines()
        .enumerate()
        .take((ta_h / lh) as usize + 1)
    {
        let row_top = ta_top + BedrockBevel::BEVEL_PX as i32 + ED_PAD_TOP + i as i32 * lh;
        let baseline = row_top + (lh as f32 * 0.78) as i32;
        draw_font_text(
            &mut d,
            font,
            line,
            font_px,
            clip.top_left.x + ED_PAD_X,
            baseline,
            ink,
            Rgb888::WHITE,
            clip,
        );
    }
    ed_draw_title(&mut d, bevel, theme, "Lotus OS -- sample.txt");
    ed_draw_menubar(&mut d, theme, None);
    ed_draw_status(
        &mut d,
        bevel,
        theme,
        "Ln 1, Col 1     sample.txt     Font: 14px",
    );

    // Dim overlay
    ed_fill(
        &mut d,
        Rectangle::new(Point::zero(), Size::new(ED_W, ED_H)),
        Rgb888::new(0x55, 0x55, 0x55),
    );

    // File picker dialog
    struct MockFs2;
    impl uefi_ui::file_picker::FileIo for MockFs2 {
        type Error = ();
        fn list(&mut self, _: &[String]) -> Result<Vec<DirEntry>, ()> {
            Ok(vec![
                DirEntry {
                    name: String::from("Documents"),
                    is_dir: true,
                },
                DirEntry {
                    name: String::from("Downloads"),
                    is_dir: true,
                },
                DirEntry {
                    name: String::from("notes.txt"),
                    is_dir: false,
                },
                DirEntry {
                    name: String::from("README.md"),
                    is_dir: false,
                },
                DirEntry {
                    name: String::from("sample.txt"),
                    is_dir: false,
                },
            ])
        }
        fn read_file(&mut self, _: &[String], _: &str) -> Result<Vec<u8>, ()> {
            Ok(vec![])
        }
        fn write_file(&mut self, _: &[String], _: &str, _: &[u8]) -> Result<(), ()> {
            Ok(())
        }
    }

    let dw = 560u32;
    let dh = 380u32;
    let dx = (ED_W - dw) / 2;
    let dy = (ED_H - dh) / 2;
    let dialog = Rectangle::new(Point::new(dx as i32, dy as i32), Size::new(dw, dh));
    let fp_lh = 18i32;
    let layout = compute_file_picker_layout(dialog, fp_lh, FP_SB_W);
    let mut dlg = FilePickerDialogState::new(PickerMode::Load, 2, &mut MockFs2).unwrap();
    dlg.picker.selected = 4;
    dlg.filename = uefi_ui::file_picker::LineInput::from_str("sample.txt");
    let scroll = ScrollbarState::new(
        ScrollAxis::Vertical,
        dlg.picker.entries.len().max(1),
        layout.visible_rows.max(1),
        0,
    );
    draw_file_picker(
        &mut d,
        bevel,
        &layout,
        &dlg.picker,
        &dlg.filename.text,
        &["All files (*.*)", "Text files (*.txt)"],
        dlg.filetype_sel,
        true,
        dlg.focus,
        fp_lh,
        6,
        &FONT_6X10,
        &theme.colors,
        &scroll,
    )
    .ok();

    save(&d, out);
}

// ─── main ────────────────────────────────────────────────────────────────────

fn main() {
    let out_root = "docs/screenshots";
    fs::create_dir_all(out_root).expect("create docs/screenshots");

    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;
    let bg = theme.colors.surface;
    let ink = theme.colors.text;

    let p = |name: &str| format!("{out_root}/{name}.png");

    eprintln!("Rendering showcase screenshots…");
    render_bevel_styles(&p("bevel_styles"), &bevel, bg, ink);
    render_buttons(&p("buttons"), &theme, &bevel);
    render_checkbox(&p("checkbox"), &theme, &bevel);
    render_radio(&p("radio_buttons"), &theme, &bevel);
    render_toggle(&p("toggle"), &theme, &bevel);
    render_slider(&p("slider"), &theme, &bevel);
    render_progress(&p("progress"), &theme, &bevel);
    render_tabs(&p("tabs"), &theme, &bevel);
    render_combobox(&p("combo_box"), &theme, &bevel);
    render_listbox(&p("list_box"), &theme, &bevel);
    render_scrollbar(&p("scrollbar"), &theme, &bevel);
    render_separators(&p("separators"), &theme, &bevel);
    render_groupbox(&p("groupbox"), &theme, &bevel);
    render_tooltip(&p("tooltip"), &theme, &bevel);
    render_hatched(&p("hatched_bg"), &theme, &bevel);
    render_status_bar(&p("status_bar"), &theme, &bevel);
    render_graph(&p("graph"), &theme, &bevel);
    render_file_picker(&p("file_picker"), &theme, &bevel);
    render_tree_view(&p("tree_view"), &theme, &bevel);
    render_keyboard_layout_picker(&p("keyboard_layout"), &theme, &bevel);

    // Editor screenshots (require Tinos font)
    match std::fs::read(TINOS_PATH) {
        Ok(font_bytes) => {
            let font =
                fontdue::Font::from_bytes(font_bytes.as_slice(), fontdue::FontSettings::default())
                    .expect("parse font");
            render_editor_screenshot(
                &font,
                14.0,
                "Lotus OS -- [untitled]",
                "",
                None,
                false,
                "",
                "",
                None,
                &theme,
                &bevel,
                &p("editor_empty"),
            );
            render_editor_screenshot(
                &font,
                14.0,
                "Lotus OS -- sample.txt",
                SAMPLE_TEXT,
                None,
                false,
                "",
                "",
                Some(5),
                &theme,
                &bevel,
                &p("editor_text"),
            );
            render_editor_screenshot(
                &font,
                14.0,
                "Lotus OS -- sample.txt",
                SAMPLE_TEXT,
                Some(0),
                false,
                "",
                "",
                None,
                &theme,
                &bevel,
                &p("editor_menu"),
            );
            render_editor_screenshot(
                &font,
                14.0,
                "Lotus OS -- sample.txt",
                SAMPLE_TEXT,
                None,
                true,
                "println",
                "3/4",
                None,
                &theme,
                &bevel,
                &p("editor_find"),
            );
            render_editor_with_filepicker(&font, &theme, &bevel, &p("editor_filepicker"));
            render_editor_screenshot(
                &font,
                20.0,
                "Lotus OS -- sample.txt",
                SAMPLE_TEXT,
                None,
                false,
                "",
                "",
                None,
                &theme,
                &bevel,
                &p("editor_large_font"),
            );
        }
        Err(e) => eprintln!("Skipping editor screenshots: {e}"),
    }

    eprintln!("Done.");
}
