//! **Showcase-only** widget strip: composes `uefi_ui` state widgets + [`uefi_ui::bedrock_controls`]
//! drawing. Not part of the library; keep demos thin and general-purpose primitives in the crate.

use alloc::format;
use alloc::string::String;

use uefi_ui::embedded_graphics::geometry::Point;
use uefi_ui::embedded_graphics::pixelcolor::Rgb888;
use uefi_ui::embedded_graphics::prelude::*;
use uefi_ui::embedded_graphics::primitives::{Line, PrimitiveStyle, Rectangle};
use uefi_ui::embedded_graphics::text::{Baseline, Text};
use uefi_ui::embedded_graphics::mono_font::{MonoFont, MonoTextStyle};
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::input::{Key, KeyEvent};
use uefi_ui::layout::pad;
use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::theme::Theme;
use uefi_ui::bedrock_controls::{
    draw_checkbox_classic, draw_combobox_chrome, draw_dropdown_glyph, draw_focus_ring,
    draw_group_box, draw_listbox_row, draw_progress_bar, draw_radio_row,
    draw_slider_track_thumb, draw_sunken_field, draw_tab_strip,
};
use uefi_ui::widgets::{
    Checkbox, DateSelect, Dropdown, LineGraph, MenuAction, MenuEntry, MenuNavigator, MenuTree,
    NumberField, ProgressBar, RadioGroup, Slider, Toggle,
};

/// List row labels for the listbox-style control.
pub const LIST_ITEMS: &[&str] = &["Alpha", "Beta", "Gamma"];

const CB: u32 = 20;           // Bedrock: 20×20 checkbox/toggle
const ROW_GAP: i32 = 6;
const LABEL_COL: i32 = 4;
const LABEL_W: i32 = 88;

/// Tab order through interactive gallery controls (Shift+Tab exits the panel in `main`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GalleryFocus {
    CheckA,
    CheckB,
    Toggle,
    Radio,
    Number,
    Slider,
    Progress,
    DemoTabs,
    Dropdown,
    List,
    Date,
    Tree,
    Graph,
}

/// Order of gallery controls within the **full-window** Tab sequence (after menu, editor, scrollbar).
pub const GALLERY_FOCUS_ORDER: &[GalleryFocus] = &[
    GalleryFocus::CheckA,
    GalleryFocus::CheckB,
    GalleryFocus::Toggle,
    GalleryFocus::Radio,
    GalleryFocus::Number,
    GalleryFocus::Slider,
    GalleryFocus::Progress,
    GalleryFocus::DemoTabs,
    GalleryFocus::Dropdown,
    GalleryFocus::List,
    GalleryFocus::Date,
    GalleryFocus::Tree,
    GalleryFocus::Graph,
];

/// Static [`MenuTree`] for the gallery strip ([`MenuNavigator`] keyboard demo).
pub static DEMO_MENU_TREE: MenuTree<'static> = MenuTree {
    roots: &[
        MenuEntry::Submenu {
            label: "File",
            children: &[
                MenuEntry::Item {
                    label: "New",
                    id: 1,
                },
                MenuEntry::Item {
                    label: "Open\u{2026}",
                    id: 2,
                },
            ],
        },
        MenuEntry::Submenu {
            label: "View",
            children: &[MenuEntry::Item {
                label: "Status",
                id: 3,
            }],
        },
        MenuEntry::Item {
            label: "Help",
            id: 4,
        },
    ],
};

impl Default for GalleryFocus {
    fn default() -> Self {
        GalleryFocus::CheckA
    }
}

impl GalleryFocus {
    /// Next control in the gallery strip (same order as [`GALLERY_FOCUS_ORDER`]).
    pub fn next(self) -> Self {
        let i = GALLERY_FOCUS_ORDER
            .iter()
            .position(|&x| x == self)
            .unwrap_or(0);
        GALLERY_FOCUS_ORDER[(i + 1) % GALLERY_FOCUS_ORDER.len()]
    }
}

pub struct GalleryState {
    pub check_a: Checkbox,
    pub check_b: Checkbox,
    pub toggle: Toggle,
    pub radio: RadioGroup,
    pub number: NumberField,
    pub slider: Slider,
    pub progress: ProgressBar,
    pub dropdown: Dropdown<'static>,
    pub list_selected: usize,
    pub date: DateSelect,
    pub graph: LineGraph,
    pub menu_line: String,
    pub fs_hint: String,
    pub tab_strip_sel: usize,
    pub tree_nav: MenuNavigator,
    pub tree_line: String,
    pub gallery_focus: GalleryFocus,
}

impl GalleryState {
    pub fn new() -> Self {
        const DD_OPTS: &[&str] = &["Red", "Green", "Blue"];
        let mut graph = LineGraph::new(16);
        for i in 0..16 {
            let t = i as f32 / 15.0;
            graph.push(t * t * 0.8 + 0.1);
        }
        Self {
            check_a: Checkbox::new(false),
            check_b: Checkbox::new(true),
            toggle: Toggle::new(false),
            radio: RadioGroup::new(3, 0),
            number: NumberField::new(42, 0, 99, 1),
            slider: Slider::new(0.0, 100.0, 33.0),
            progress: ProgressBar::new(0.65),
            dropdown: Dropdown::new(DD_OPTS, 1),
            list_selected: 0,
            date: DateSelect::new(2026, 3, 27),
            graph,
            menu_line: String::from("Menu: —"),
            fs_hint: String::from("Volume: —"),
            tab_strip_sel: 1,
            tree_nav: MenuNavigator::default(),
            tree_line: String::from("Tree: —"),
            gallery_focus: GalleryFocus::default(),
        }
    }
}

/// Geometry for painting and hit-testing (must stay in sync with [`paint_gallery`]).
pub struct GalleryLayout {
    pub check_a: Rectangle,
    pub check_b: Rectangle,
    pub toggle_track: Rectangle,
    pub radio_row: Rectangle,
    pub number_field: Rectangle,
    pub slider_track: Rectangle,
    pub progress_track: Rectangle,
    pub tab_strip: Rectangle,
    pub dropdown_field: Rectangle,
    pub dropdown_btn: Rectangle,
    pub dropdown_popup: Option<Rectangle>,
    pub list_box: Rectangle,
    pub date_row: Rectangle,
    pub tree_row: Rectangle,
    pub graph: Rectangle,
}

fn row_h(line_h: i32) -> i32 {
    line_h.max(15)
}

fn tree_status_line(nav: &MenuNavigator) -> String {
    let t = &DEMO_MENU_TREE;
    let Some(root) = t.roots.get(nav.root_index) else {
        return String::from("Tree: —");
    };
    let root_lbl = match root {
        MenuEntry::Item { label, .. } | MenuEntry::Submenu { label, .. } => *label,
    };
    if let Some((ri, si)) = nav.sub {
        if let Some(MenuEntry::Submenu { label: rl, children }) = t.roots.get(ri) {
            if let Some(ch) = children.get(si) {
                let sub_l = match ch {
                    MenuEntry::Item { label, .. } | MenuEntry::Submenu { label, .. } => *label,
                };
                return format!("Tree: {} › {}", rl, sub_l);
            }
        }
    }
    format!("Tree: {}", root_lbl)
}

/// Computes layout for the gallery panel content.
pub fn compute_gallery_layout(inner: &Rectangle, line_h: i32, g: &GalleryState) -> GalleryLayout {
    let lh = row_h(line_h);
    let x0 = inner.top_left.x + LABEL_COL;
    let mut y = inner.top_left.y + 4;
    let full_w = inner.size.width.saturating_sub(8) as i32;

    y += lh + ROW_GAP;

    let check_a = Rectangle::new(Point::new(x0 + LABEL_W, y), Size::new(CB, CB));
    let check_b = Rectangle::new(Point::new(x0 + LABEL_W + 96, y), Size::new(CB, CB));
    y += lh + ROW_GAP;

    let toggle_track = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new(40, CB),
    );
    y += lh + ROW_GAP;

    // 3 × 20px circles + 2 × 8px gaps = 76px wide, 20px tall
    let radio_row = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((20 * 3 + 8 * 2) as u32, 20),
    );
    y += lh + ROW_GAP;

    let number_field = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new(52, lh as u32),
    );
    y += lh + ROW_GAP;

    let slider_track = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((full_w - LABEL_W - 8).max(48) as u32, 14),
    );
    y += 14 + ROW_GAP;

    let progress_track = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((full_w - LABEL_W - 8).max(48) as u32, 12),
    );
    y += 12 + ROW_GAP;

    let tab_w = (full_w - LABEL_W - 8).max(120) as u32;
    // Bedrock: inactive tab = 36px, active raised +4 = 40px; allocate 44px so raised tab fits
    let tab_strip = Rectangle::new(Point::new(x0 + LABEL_W, y + 4), Size::new(tab_w, 36));
    y += 44 + ROW_GAP;

    let dd_w = (full_w - LABEL_W - 8).max(100) as u32;
    let dropdown_field = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new(dd_w.saturating_sub(30), lh as u32),
    );
    // Bedrock: dropdown arrow button = 30px wide
    let dropdown_btn = Rectangle::new(
        Point::new(
            dropdown_field.top_left.x + dropdown_field.size.width as i32,
            y,
        ),
        Size::new(30, lh as u32),
    );
    let dropdown_popup = if g.dropdown.open {
        let n = g.dropdown.options.len().max(1);
        let ph = (n as i32 * lh + 6) as u32;
        Some(Rectangle::new(
            Point::new(dropdown_field.top_left.x, y + lh + 2),
            Size::new(
                dropdown_field.size.width + dropdown_btn.size.width,
                ph,
            ),
        ))
    } else {
        None
    };
    y += lh + ROW_GAP;
    if g.dropdown.open {
        let h = g.dropdown.options.len() as i32 * lh + 6;
        y += h + ROW_GAP;
    }

    let list_box = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((full_w - LABEL_W - 8).max(80) as u32, (3 * lh + 6) as u32),
    );
    y += 3 * lh + 6 + ROW_GAP;

    let date_row = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((full_w - LABEL_W - 8).max(160) as u32, lh as u32),
    );
    y += lh + ROW_GAP;

    let tree_h = lh * 2 + 4;
    let tree_row = Rectangle::new(
        Point::new(x0 + LABEL_W, y),
        Size::new((full_w - LABEL_W - 8).max(160) as u32, tree_h as u32),
    );
    y += tree_h + ROW_GAP;

    y += lh + ROW_GAP + lh + ROW_GAP;

    let gh = 28i32;
    let graph = Rectangle::new(
        Point::new(x0 + LABEL_W - 4, y),
        Size::new((full_w - LABEL_W).max(100) as u32, gh as u32),
    );

    GalleryLayout {
        check_a,
        check_b,
        toggle_track,
        radio_row,
        number_field,
        slider_track,
        progress_track,
        tab_strip,
        dropdown_field,
        dropdown_btn,
        dropdown_popup,
        list_box,
        date_row,
        tree_row,
        graph,
    }
}

fn draw_focus_if(
    target: &mut BgrxFramebuffer<'_>,
    panel_ok: bool,
    gf: GalleryFocus,
    want: GalleryFocus,
    r: Rectangle,
    c: Rgb888,
) {
    if panel_ok && gf == want {
        let _ = draw_focus_ring(target, r, c);
    }
}

fn radio_hit_index(px: i32, py: i32, row: Rectangle, count: usize) -> Option<usize> {
    if !row.contains(Point::new(px, py)) {
        return None;
    }
    // Bedrock: 20px circle + 8px gap
    let d = 20i32;
    let gap = 8i32;
    let mut x = row.top_left.x;
    for i in 0..count {
        let rr = Rectangle::new(Point::new(x, row.top_left.y), Size::new((d + gap) as u32, row.size.height));
        if rr.contains(Point::new(px, py)) {
            return Some(i);
        }
        x += d + gap;
    }
    None
}

/// Keyboard when the gallery panel is focused: routes by [`GalleryState::gallery_focus`].
pub fn apply_gallery_key_event(
    g: &mut GalleryState,
    ev: &KeyEvent,
    _virtual_shift: bool,
) -> bool {
    let key = ev.key;

    if g.dropdown.open {
        let _ = g.dropdown.apply_key_event(ev);
        return true;
    }

    if g.gallery_focus == GalleryFocus::Dropdown {
        let _ = g.dropdown.apply_key_event(ev);
        return true;
    }

    match g.gallery_focus {
        GalleryFocus::CheckA => {
            if matches!(key, Key::Enter) || matches!(key, Key::Character(' ')) {
                g.check_a.toggle();
                return true;
            }
        }
        GalleryFocus::CheckB => {
            if matches!(key, Key::Enter) || matches!(key, Key::Character(' ')) {
                g.check_b.toggle();
                return true;
            }
        }
        GalleryFocus::Toggle => {
            if matches!(key, Key::Enter) || matches!(key, Key::Character(' ')) {
                g.toggle.flip();
                return true;
            }
        }
        GalleryFocus::Radio => match key {
            Key::Left => {
                g.radio.prev();
                return true;
            }
            Key::Right => {
                g.radio.next();
                return true;
            }
            Key::Enter => {
                g.radio.next();
                return true;
            }
            Key::Character(' ') => {
                g.radio.next();
                return true;
            }
            _ => {}
        },
        GalleryFocus::Number => {
            if g.number.apply_key_event(ev) {
                return true;
            }
        }
        GalleryFocus::Slider => match key {
            Key::Left => {
                g.slider.value = (g.slider.value - 2.5_f32).max(g.slider.min);
                return true;
            }
            Key::Right => {
                g.slider.value = (g.slider.value + 2.5_f32).min(g.slider.max);
                return true;
            }
            Key::Home => {
                g.slider.value = g.slider.min;
                return true;
            }
            Key::End => {
                g.slider.value = g.slider.max;
                return true;
            }
            _ => {}
        },
        GalleryFocus::Progress => {
            if matches!(key, Key::Enter) || matches!(key, Key::Character(' ')) {
                g.progress.set(1.0 - g.progress.value);
                return true;
            }
        }
        GalleryFocus::DemoTabs => match key {
            Key::Left => {
                g.tab_strip_sel = g.tab_strip_sel.saturating_sub(1);
                return true;
            }
            Key::Right => {
                g.tab_strip_sel = (g.tab_strip_sel + 1).min(2);
                return true;
            }
            _ => {}
        },
        GalleryFocus::Tree => {
            if let Some(act) = g.tree_nav.apply_key_event(&DEMO_MENU_TREE, ev) {
                match act {
                    MenuAction::Activated(id) => {
                        g.tree_line = format!("Tree: activated id {}", id);
                    }
                    MenuAction::SubmenuOpened | MenuAction::SubmenuClosed | MenuAction::Moved => {
                        g.tree_line = tree_status_line(&g.tree_nav);
                    }
                }
                return true;
            }
        },
        GalleryFocus::List => match key {
            Key::Up => {
                if g.list_selected > 0 {
                    g.list_selected -= 1;
                }
                return true;
            }
            Key::Down => {
                if g.list_selected + 1 < LIST_ITEMS.len() {
                    g.list_selected += 1;
                }
                return true;
            }
            Key::Home => {
                g.list_selected = 0;
                return true;
            }
            Key::End => {
                g.list_selected = LIST_ITEMS.len().saturating_sub(1);
                return true;
            }
            _ => {}
        },
        GalleryFocus::Date => {
            if matches!(key, Key::Tab) {
                return false;
            }
            let _ = g.date.apply_key_event(ev);
            return true;
        }
        GalleryFocus::Graph => {}
        GalleryFocus::Dropdown => {}
    }

    match key {
        Key::Character('+') | Key::Character('=') => {
            g.slider.value = (g.slider.value + 5.0_f32).min(g.slider.max);
            true
        }
        Key::Character('-') | Key::Character('_') => {
            g.slider.value = (g.slider.value - 5.0_f32).max(g.slider.min);
            true
        }
        _ => false,
    }
}

/// Pointer pick: returns `true` if the event was handled (caller sets gallery focus to Gallery).
pub fn gallery_pointer_down(
    g: &mut GalleryState,
    px: i32,
    py: i32,
    inner: &Rectangle,
    line_h: i32,
) -> bool {
    let pt = Point::new(px, py);
    if !inner.contains(pt) {
        return false;
    }
    let layout = compute_gallery_layout(inner, line_h, g);

    if let Some(pop) = layout.dropdown_popup {
        if pop.contains(pt) && g.dropdown.open {
            g.gallery_focus = GalleryFocus::Dropdown;
            let lh = row_h(line_h);
            let j = ((py - pop.top_left.y - 3) / lh).max(0) as usize;
            if j < g.dropdown.options.len() {
                g.dropdown.set_menu_focus_index(j);
                g.dropdown.selected = j;
                g.dropdown.open = false;
            }
            return true;
        }
    }

    if layout.dropdown_btn.contains(pt) {
        g.gallery_focus = GalleryFocus::Dropdown;
        g.dropdown.toggle_open();
        return true;
    }
    if layout.dropdown_field.contains(pt) {
        g.gallery_focus = GalleryFocus::Dropdown;
        g.dropdown.toggle_open();
        return true;
    }

    if layout.check_a.contains(pt) {
        g.gallery_focus = GalleryFocus::CheckA;
        g.check_a.toggle();
        return true;
    }
    if layout.check_b.contains(pt) {
        g.gallery_focus = GalleryFocus::CheckB;
        g.check_b.toggle();
        return true;
    }
    if layout.toggle_track.contains(pt) {
        g.gallery_focus = GalleryFocus::Toggle;
        g.toggle.flip();
        return true;
    }
    if let Some(idx) = radio_hit_index(px, py, layout.radio_row, g.radio.count) {
        g.gallery_focus = GalleryFocus::Radio;
        g.radio.select(idx);
        return true;
    }
    if layout.number_field.contains(pt) {
        g.gallery_focus = GalleryFocus::Number;
        return true;
    }
    if layout.slider_track.contains(pt) {
        g.gallery_focus = GalleryFocus::Slider;
        let inner_t = pad(layout.slider_track, 2);
        let t = (px - inner_t.top_left.x) as f32 / inner_t.size.width.max(1) as f32;
        g.slider.set_from_ratio(t.clamp(0.0, 1.0));
        return true;
    }
    if layout.progress_track.contains(pt) {
        g.gallery_focus = GalleryFocus::Progress;
        return true;
    }
    if layout.tab_strip.contains(pt) {
        g.gallery_focus = GalleryFocus::DemoTabs;
        let w = layout.tab_strip.size.width as i32;
        let rel = px - layout.tab_strip.top_left.x;
        g.tab_strip_sel = if w > 0 {
            (rel * 3 / w).clamp(0, 2) as usize
        } else {
            0
        };
        return true;
    }
    if layout.list_box.contains(pt) {
        g.gallery_focus = GalleryFocus::List;
        let lh = row_h(line_h);
        let j = ((py - layout.list_box.top_left.y - 3) / lh).max(0) as usize;
        if j < LIST_ITEMS.len() {
            g.list_selected = j;
        }
        return true;
    }
    if layout.date_row.contains(pt) {
        g.gallery_focus = GalleryFocus::Date;
        return true;
    }
    if layout.tree_row.contains(pt) {
        g.gallery_focus = GalleryFocus::Tree;
        return true;
    }
    if layout.graph.contains(pt) {
        g.gallery_focus = GalleryFocus::Graph;
        return true;
    }

    g.gallery_focus = GalleryFocus::CheckA;
    true
}

fn draw_label(
    target: &mut BgrxFramebuffer<'_>,
    font: &MonoFont<'_>,
    fg: Rgb888,
    x: i32,
    y: i32,
    s: &str,
) {
    let style = MonoTextStyle::new(font, fg);
    let _ = Text::with_baseline(s, Point::new(x, y), style, Baseline::Top)
        .draw(target)
        .ok();
}

/// Paint Bedrock-style widgets inside `inner`.
#[allow(clippy::too_many_arguments)]
pub fn paint_gallery(
    target: &mut BgrxFramebuffer<'_>,
    theme: &Theme,
    bevel: &BedrockBevel,
    font: &MonoFont<'_>,
    g: &GalleryState,
    inner: &Rectangle,
    line_h: i32,
    panel_focused: bool,
) {
    // DC-46: bevel is now drawn by the caller (scene.rs) on gallery_rect; no bevel here.

    let lh = row_h(line_h);
    let layout = compute_gallery_layout(inner, line_h, g);
    let x0 = inner.top_left.x + LABEL_COL;
    let y0 = inner.top_left.y + 4;
    let fg = theme.colors.text;
    let focus_c = theme.colors.border_focus;

    draw_label(target, font, fg, x0, y0, "Widget gallery");
    let mut y = y0 + lh + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Checkbox");
    let paper = Rgb888::new(0xff, 0xff, 0xff);
    let _ = draw_checkbox_classic(target, bevel, layout.check_a, g.check_a.checked(), paper, true);
    let _ = draw_checkbox_classic(target, bevel, layout.check_b, g.check_b.checked(), paper, true);
    draw_focus_if(target, panel_focused, g.gallery_focus, GalleryFocus::CheckA, layout.check_a, focus_c);
    draw_focus_if(target, panel_focused, g.gallery_focus, GalleryFocus::CheckB, layout.check_b, focus_c);
    y += lh + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Toggle");
    let tpaper = if g.toggle.on {
        theme.colors.selection_bg
    } else {
        paper
    };
    let _ = draw_sunken_field(target, bevel, layout.toggle_track, tpaper);
    // Text on navy background must be caption_on_accent (white); dark fg on canvas.
    let toggle_fg = if g.toggle.on { theme.colors.caption_on_accent } else { fg };
    draw_label(
        target,
        font,
        toggle_fg,
        layout.toggle_track.top_left.x + 6,
        layout.toggle_track.top_left.y + 2,
        if g.toggle.on { "ON" } else { "OFF" },
    );
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Toggle,
        layout.toggle_track,
        focus_c,
    );
    y += lh + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Radio");
    let _ = draw_radio_row(
        target,
        layout.radio_row,
        g.radio.count,
        g.radio.selected,
        bevel,
        true,
    );
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Radio,
        layout.radio_row,
        focus_c,
    );
    y += lh + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Number");
    let _ = draw_sunken_field(target, bevel, layout.number_field, paper);
    let nfill = pad(layout.number_field, 3);
    draw_label(
        target,
        font,
        fg,
        nfill.top_left.x,
        nfill.top_left.y + 1,
        &format!("{}", g.number.value),
    );
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Number,
        layout.number_field,
        focus_c,
    );
    y += lh + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Slider");
    let _ = draw_slider_track_thumb(target, bevel, layout.slider_track, g.slider.ratio(), 8);
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Slider,
        layout.slider_track,
        focus_c,
    );
    y += 14 + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Progress");
    let _ = draw_progress_bar(
        target,
        bevel,
        layout.progress_track,
        g.progress.value,
        theme.colors.progress_track,
        theme.colors.progress_fill,
    );
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Progress,
        layout.progress_track,
        focus_c,
    );
    y += 12 + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Tabs");
    let _ = draw_tab_strip(
        target,
        bevel,
        layout.tab_strip.top_left,
        36,
        &[50, 50, 60],
        g.tab_strip_sel.min(2),
        theme.colors.surface,
    );
    let tab_labels = ["One", "Two", "Three"];
    let sel = g.tab_strip_sel.min(2);
    let mut tx = layout.tab_strip.top_left.x;
    let widths = [50i32, 50, 60];
    for (i, name) in tab_labels.iter().enumerate() {
        let ty = if i == sel {
            layout.tab_strip.top_left.y - 4 + 10  // active tab raised 4px
        } else {
            layout.tab_strip.top_left.y + 10
        };
        draw_label(target, font, fg, tx + 6, ty, name);
        tx += widths[i] + 2;
    }
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::DemoTabs,
        layout.tab_strip,
        focus_c,
    );
    y += 44 + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Combo box");
    let _ = draw_combobox_chrome(
        target,
        bevel,
        layout.dropdown_field,
        layout.dropdown_btn,
        paper,
    );
    // DC-48: use pad(…, 3) to position label inside the 3px-deep sunken border
    let dfill = pad(layout.dropdown_field, 3);
    draw_label(
        target,
        font,
        fg,
        dfill.top_left.x + 2,
        dfill.top_left.y + 1,
        g.dropdown.options[g.dropdown.selected],
    );
    // DC-47: draw pixel triangle instead of "▼" (not in FONT_6X10)
    let _ = draw_dropdown_glyph(target, layout.dropdown_btn, fg);
    let combo_union = Rectangle::new(
        layout.dropdown_field.top_left,
        Size::new(
            layout.dropdown_field.size.width + layout.dropdown_btn.size.width,
            layout.dropdown_field.size.height,
        ),
    );
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Dropdown,
        combo_union,
        focus_c,
    );

    if let Some(pop) = layout.dropdown_popup {
        // DC-39: window-style border; DC-42/45: fill inner pad(3) with canvas white
        let _ = bevel.draw_raised_soft(target, pop);
        let pop_inner = pad(pop, 3);
        target.fill_rect_solid(
            pop_inner.top_left.x as u32,
            pop_inner.top_left.y as u32,
            pop_inner.size.width,
            pop_inner.size.height,
            Rgb888::new(0xff, 0xff, 0xff),
        );
        for (j, opt) in g.dropdown.options.iter().enumerate() {
            let row_y = pop_inner.top_left.y + j as i32 * lh;
            let sel = g.dropdown.open && j == g.dropdown.menu_focus_index();
            let bg = if sel {
                theme.colors.selection_bg
            } else {
                Rgb888::new(0xff, 0xff, 0xff)
            };
            let item_fg = if sel { theme.colors.caption_on_accent } else { theme.colors.text };
            target.fill_rect_solid(
                pop_inner.top_left.x as u32,
                row_y as u32,
                pop_inner.size.width,
                lh as u32,
                bg,
            );
            draw_label(target, font, item_fg, pop_inner.top_left.x + 4, row_y, opt);
        }
    }
    y += lh + ROW_GAP;
    if g.dropdown.open {
        y += g.dropdown.options.len() as i32 * lh + 6 + ROW_GAP;
    }

    draw_label(target, font, fg, x0, y, "List box");
    let _ = draw_sunken_field(target, bevel, layout.list_box, paper);
    let lfill = pad(layout.list_box, 2);
    for (j, name) in LIST_ITEMS.iter().enumerate() {
        let row_y = lfill.top_left.y + 3 + j as i32 * lh;
        let row_sel = j == g.list_selected;
        let list_focused = panel_focused && g.gallery_focus == GalleryFocus::List;
        let _ = draw_listbox_row(
            target,
            lfill,
            row_y,
            lh,
            row_sel,
            list_focused && j == g.list_selected,
            paper,
            theme.colors.selection_bg,
        );
        // White text on navy selected row; dark text otherwise.
        let row_fg = if row_sel { theme.colors.caption_on_accent } else { fg };
        draw_label(target, font, row_fg, lfill.top_left.x + 4, row_y, name);
    }
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::List,
        layout.list_box,
        focus_c,
    );
    y += 3 * lh + 6 + ROW_GAP;

    draw_label(target, font, fg, x0, y, "Date");
    let _ = draw_sunken_field(target, bevel, layout.date_row, paper);
    let df = pad(layout.date_row, 2);
    let ds = format!(
        "{}-{:02}-{:02}  ({:?})",
        g.date.year, g.date.month, g.date.day, g.date.focus
    );
    draw_label(target, font, fg, df.top_left.x + 4, df.top_left.y + 1, &ds);
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Date,
        layout.date_row,
        focus_c,
    );

    draw_label(target, font, fg, x0, layout.tree_row.top_left.y, "Menu tree");
    let _ = draw_sunken_field(target, bevel, layout.tree_row, paper);
    let tfill = pad(layout.tree_row, 2);
    let line1_y = tfill.top_left.y + 3;
    let mut tx = tfill.top_left.x + 4;
    for (i, ent) in DEMO_MENU_TREE.roots.iter().enumerate() {
        let label = match ent {
            MenuEntry::Item { label, .. } | MenuEntry::Submenu { label, .. } => *label,
        };
        let sel = i == g.tree_nav.root_index && g.tree_nav.sub.is_none();
        let bg = if sel { theme.colors.selection_bg } else { paper };
        let item_fg = if sel { theme.colors.caption_on_accent } else { fg };
        let tw = (label.chars().count() as i32 * 6 + 8).max(24) as u32;
        let tr = Rectangle::new(Point::new(tx, line1_y - 1), Size::new(tw, lh as u32));
        Rectangle::new(tr.top_left, tr.size)
            .into_styled(PrimitiveStyle::with_fill(bg))
            .draw(target)
            .ok();
        draw_label(target, font, item_fg, tx + 2, line1_y, label);
        tx += tw as i32 + 6;
    }
    if let Some((ri, si)) = g.tree_nav.sub {
        if let Some(MenuEntry::Submenu { children, .. }) = DEMO_MENU_TREE.roots.get(ri) {
            let line2_y = line1_y + lh + 2;
            let mut sx = tfill.top_left.x + 8;
            for (j, ch) in children.iter().enumerate() {
                let label = match ch {
                    MenuEntry::Item { label, .. } | MenuEntry::Submenu { label, .. } => *label,
                };
                let sub_sel = j == si;
                let bg = if sub_sel { theme.colors.selection_bg } else { paper };
                let sub_fg = if sub_sel { theme.colors.caption_on_accent } else { fg };
                let sw = (label.chars().count() as i32 * 6 + 8).max(24) as u32;
                let sr = Rectangle::new(Point::new(sx, line2_y - 1), Size::new(sw, lh as u32));
                Rectangle::new(sr.top_left, sr.size)
                    .into_styled(PrimitiveStyle::with_fill(bg))
                    .draw(target)
                    .ok();
                draw_label(target, font, sub_fg, sx + 2, line2_y, label);
                sx += sw as i32 + 6;
            }
        }
    }
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Tree,
        layout.tree_row,
        focus_c,
    );

    let status_y = layout.tree_row.top_left.y + layout.tree_row.size.height as i32 + ROW_GAP;
    draw_label(target, font, fg, x0, status_y, &g.menu_line);
    draw_label(target, font, fg, x0, status_y + lh + 2, &g.fs_hint);
    draw_label(target, font, fg, x0, status_y + (lh + 2) * 2, &g.tree_line);

    // T-03: use GroupBox (etched Bedrock grouping style) around the sparkline graph.
    // Fill with surface color first (draw_groupbox doesn't fill), then draw etched border.
    Rectangle::new(layout.graph.top_left, layout.graph.size)
        .into_styled(PrimitiveStyle::with_fill(theme.colors.surface))
        .draw(target)
        .ok();
    // Label gap: leave room for "Graph" text (6 chars × 6px + 8px padding ≈ 44px)
    let gap_x = layout.graph.top_left.x + 8;
    let _ = draw_group_box(target, bevel, layout.graph, Some((gap_x, 44)), theme.colors.surface);
    draw_label(target, font, fg, gap_x + 2, layout.graph.top_left.y - lh / 2, "Graph");
    let inner_g = pad(layout.graph, 2);
    let pts = g.graph.points(inner_g);
    for w in pts.windows(2) {
        Line::new(w[0], w[1])
            .into_styled(PrimitiveStyle::with_stroke(theme.colors.graph_line, 1))
            .draw(target)
            .ok();
    }
    draw_focus_if(
        target,
        panel_focused,
        g.gallery_focus,
        GalleryFocus::Graph,
        layout.graph,
        focus_c,
    );
}
