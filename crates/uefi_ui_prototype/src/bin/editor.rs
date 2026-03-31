//! Bedrock-style text editor (plain text editor).
//!
//! Run: `cargo run -p uefi_ui_prototype --bin editor --features sdl`
//!
//! Keys:
//!   Ctrl+N  New       Ctrl+O  Open      Ctrl+S  Save    Ctrl+Shift+S  Save As
//!   Ctrl+X  Cut       Ctrl+C  Copy      Ctrl+V  Paste   Ctrl+A  Select All
//!   Ctrl+F  Find      F3 / Enter  Next match   Escape  Close bar / cancel
//!   F11     Toggle fullscreen
//!   Ctrl++  / Ctrl+-  Increase / decrease font size

#![allow(clippy::too_many_arguments)]

use std::{collections::HashMap, process::Termination};

use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle},
    text::{Baseline, Text},
};
use embedded_graphics_simulator::{
    sdl2::{Keycode, Mod},
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};

use uefi_ui::{
    bedrock::BedrockBevel,
    bedrock_controls::{
        compute_file_picker_layout, draw_file_picker, draw_menu_bar, draw_menu_popup,
        draw_scrollbar_vertical, draw_status_border, draw_sunken_field, draw_title_button, FP_SB_W,
    },
    file_picker::{
        DirEntry, FileIo, FilePickerDialogAction, FilePickerDialogState, LineInput, PickerMode,
    },
    input::{Key, KeyEvent, Modifiers},
    layout::pad,
    theme::Theme,
    widgets::{ScrollAxis, ScrollbarState, TextArea},
};

// ── constants ─────────────────────────────────────────────────────────────────

const WIN_W: u32 = 900;
const WIN_H: u32 = 660;
const TITLE_H: u32 = 24;
const MENUBAR_H: u32 = 22;
const STATUSBAR_H: u32 = 22;
const FINDBAR_H: u32 = 28;
const SB_W: u32 = 17; // vertical scrollbar width
const EDITOR_PAD_X: i32 = 6; // horizontal text margin inside the textarea
const EDITOR_PAD_TOP: i32 = 4;

const FONT_SIZES: &[f32] = &[10.0, 12.0, 14.0, 16.0, 18.0, 20.0];
const DEFAULT_FONT_SIZE_IDX: usize = 2; // 14px

const TINOS_PATH: &str = "../../../../assets/fonts/Tinos-Regular.ttf";

// ── glyph cache ───────────────────────────────────────────────────────────────

#[derive(Hash, PartialEq, Eq)]
struct GlyphKey(u32, u32); // (char as u32, px*10 as u32)

struct CachedGlyph {
    w: usize,
    h: usize,
    /// Distance from baseline to top of glyph bitmap (positive = above baseline).
    above_baseline: i32,
    advance: i32,
    bitmap: Vec<u8>,
}

struct GlyphCache {
    font: fontdue::Font,
    px: f32,
    map: HashMap<GlyphKey, CachedGlyph>,
}

impl GlyphCache {
    fn new(font: fontdue::Font, px: f32) -> Self {
        Self {
            font,
            px,
            map: HashMap::new(),
        }
    }

    fn set_size(&mut self, px: f32) {
        if (px - self.px).abs() > 0.01 {
            self.map.clear();
            self.px = px;
        }
    }

    fn get(&mut self, c: char) -> &CachedGlyph {
        let key = GlyphKey(c as u32, (self.px * 10.0) as u32);
        let px = self.px;
        let font = &self.font;
        self.map.entry(key).or_insert_with(|| {
            let (m, bmp) = font.rasterize(c, px);
            CachedGlyph {
                w: m.width,
                h: m.height,
                above_baseline: m.ymin + m.height as i32,
                advance: m.advance_width.round() as i32,
                bitmap: bmp,
            }
        })
    }

    fn line_height(&self) -> i32 {
        (self.px * 1.4).ceil() as i32
    }

    /// Approximate advance width for a character (for layout estimates).
    fn char_advance(&mut self, c: char) -> i32 {
        self.get(c).advance
    }

    /// Pixel width of a string.
    fn str_width(&mut self, s: &str) -> i32 {
        s.chars().map(|c| self.char_advance(c)).sum()
    }
}

// ── text rendering helpers ────────────────────────────────────────────────────

fn blend(bg: Rgb888, fg: Rgb888, a: u8) -> Rgb888 {
    let b = a as u16;
    let ib = 255 - b;
    Rgb888::new(
        ((fg.r() as u16 * b + bg.r() as u16 * ib) / 255) as u8,
        ((fg.g() as u16 * b + bg.g() as u16 * ib) / 255) as u8,
        ((fg.b() as u16 * b + bg.b() as u16 * ib) / 255) as u8,
    )
}

/// Draw a single line of text using fontdue glyphs.
/// `baseline_y` = y-coordinate of the text baseline.
/// Returns the x position after the last character.
fn draw_glyph_line(
    display: &mut SimulatorDisplay<Rgb888>,
    cache: &mut GlyphCache,
    text: &str,
    x0: i32,
    baseline_y: i32,
    fg: Rgb888,
    bg: Rgb888,
    clip: Rectangle,
) -> i32 {
    let mut x = x0;
    let disp_w = display.size().width as i32;
    let disp_h = display.size().height as i32;

    for ch in text.chars() {
        if x >= clip.top_left.x + clip.size.width as i32 {
            break;
        }
        let g = cache.get(ch);
        let gx = x;
        let gy = baseline_y - g.above_baseline;
        let adv = g.advance;
        let gw = g.w;
        let gh = g.h;

        for row in 0..gh {
            for col in 0..gw {
                let alpha = g.bitmap[row * gw + col];
                if alpha == 0 {
                    continue;
                }
                let px = gx + col as i32;
                let py = gy + row as i32;
                if px < clip.top_left.x || px >= clip.top_left.x + clip.size.width as i32 {
                    continue;
                }
                if py < clip.top_left.y || py >= clip.top_left.y + clip.size.height as i32 {
                    continue;
                }
                if px < 0 || px >= disp_w || py < 0 || py >= disp_h {
                    continue;
                }
                let c = blend(bg, fg, alpha);
                Pixel(Point::new(px, py), c).draw(display).ok();
            }
        }
        x += adv;
    }
    x
}

// ── file I/O ──────────────────────────────────────────────────────────────────

struct StdFileIo;

impl FileIo for StdFileIo {
    type Error = std::io::Error;

    fn list(&mut self, path: &[String]) -> Result<Vec<DirEntry>, Self::Error> {
        let dir = path_from_parts(path);
        let mut entries: Vec<DirEntry> = std::fs::read_dir(&dir)?
            .filter_map(|e| e.ok())
            .map(|e| {
                let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                DirEntry {
                    name: e.file_name().to_string_lossy().into_owned(),
                    is_dir,
                }
            })
            .collect();
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        Ok(entries)
    }

    fn read_file(&mut self, path: &[String], name: &str) -> Result<Vec<u8>, Self::Error> {
        let mut p = path_from_parts(path);
        p.push(name);
        std::fs::read(p)
    }

    fn write_file(&mut self, path: &[String], name: &str, data: &[u8]) -> Result<(), Self::Error> {
        let mut p = path_from_parts(path);
        p.push(name);
        std::fs::write(p, data)
    }
}

fn path_from_parts(parts: &[String]) -> std::path::PathBuf {
    if parts.is_empty() {
        return std::path::PathBuf::from("/");
    }
    parts.iter().collect()
}

// ── application state ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilePickerReason {
    Open,
    SaveAs,
}

enum AppMode {
    Editing,
    FilePicker {
        reason: FilePickerReason,
        state: Box<FilePickerDialogState>,
    },
}

struct FindBar {
    visible: bool,
    query: LineInput,
    /// Byte (lo, hi) pairs of current matches in textarea.text.
    matches: Vec<(usize, usize)>,
    current: usize,
}

impl FindBar {
    fn new() -> Self {
        Self {
            visible: false,
            query: LineInput::new(),
            matches: Vec::new(),
            current: 0,
        }
    }

    fn search(&mut self, text: &str) {
        self.matches.clear();
        self.current = 0;
        let q = &self.query.text;
        if q.is_empty() {
            return;
        }
        let mut start = 0;
        while let Some(pos) = text[start..].find(q.as_str()) {
            let lo = start + pos;
            let hi = lo + q.len();
            self.matches.push((lo, hi));
            start = lo + 1;
            if start >= text.len() {
                break;
            }
        }
    }
}

struct MenuState {
    /// Index of open menu (0=File, 1=Edit, 2=View), or None.
    open: Option<usize>,
    popup_sel: usize,
    active: bool,
}

impl MenuState {
    fn new() -> Self {
        Self {
            open: None,
            popup_sel: 0,
            active: false,
        }
    }
}

// Menu item lists
const FILE_ITEMS: &[&str] = &[
    "New\tCtrl+N",
    "Open...\tCtrl+O",
    "Save\tCtrl+S",
    "Save As...",
    "—",
    "Exit",
];
const EDIT_ITEMS: &[&str] = &[
    "Cut\tCtrl+X",
    "Copy\tCtrl+C",
    "Paste\tCtrl+V",
    "Delete",
    "—",
    "Find...\tCtrl+F",
    "Select All\tCtrl+A",
];
const VIEW_ITEMS: &[&str] = &["Larger Font  (+)", "Smaller Font  (-)"];

fn menu_items(idx: usize) -> &'static [&'static str] {
    match idx {
        0 => FILE_ITEMS,
        1 => EDIT_ITEMS,
        _ => VIEW_ITEMS,
    }
}

struct EditorLayout {
    title_bar: Rectangle,
    menu_bar: Rectangle,
    /// Menu cells for draw_menu_bar (File / Edit / View).
    menu_cells: [Rectangle; 3],
    textarea: Rectangle,
    scrollbar: Rectangle,
    find_bar: Rectangle, // valid when find_visible
    status_bar: Rectangle,
    line_height: i32,
    visible_lines: usize,
}

struct EditorApp {
    textarea: TextArea,
    filepath: Option<String>,
    dirty: bool,

    mode: AppMode,
    menu: MenuState,
    find: FindBar,
    clipboard: String,

    cache: GlyphCache,
    font_size_idx: usize,

    theme: Theme,
    bevel: BedrockBevel,
    layout: EditorLayout,

    win_w: u32,
    win_h: u32,
    fullscreen: bool,

    std_io: StdFileIo,
}

impl EditorApp {
    fn new(font: fontdue::Font) -> Self {
        let theme = Theme::bedrock_classic();
        let bevel = BedrockBevel::CLASSIC;
        let font_size = FONT_SIZES[DEFAULT_FONT_SIZE_IDX];
        let cache = GlyphCache::new(font, font_size);
        let layout = build_layout(WIN_W, WIN_H, cache.line_height(), false);
        Self {
            textarea: TextArea::new(),
            filepath: None,
            dirty: false,
            mode: AppMode::Editing,
            menu: MenuState::new(),
            find: FindBar::new(),
            clipboard: String::new(),
            cache,
            font_size_idx: DEFAULT_FONT_SIZE_IDX,
            theme,
            bevel,
            layout,
            win_w: WIN_W,
            win_h: WIN_H,
            fullscreen: false,
            std_io: StdFileIo,
        }
    }

    fn rebuild_layout(&mut self) {
        self.layout = build_layout(
            self.win_w,
            self.win_h,
            self.cache.line_height(),
            self.find.visible,
        );
        let vis = self.layout.visible_lines.max(1);
        self.textarea.scroll_to_cursor(vis);
    }

    fn window_title(&self) -> String {
        let name = self
            .filepath
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        let dirty = if self.dirty { "* " } else { "" };
        format!("{dirty}{name} — Text Editor")
    }

    // ── drawing ────────────────────────────────────────────────────────────────

    fn draw(&mut self, display: &mut SimulatorDisplay<Rgb888>) {
        let bg = self.theme.colors.surface;
        fill_rect(display, Rectangle::new(Point::zero(), display.size()), bg);

        self.draw_title_bar(display);
        self.draw_menu_bar(display);
        self.draw_textarea(display);
        self.draw_scrollbar(display);
        if self.find.visible {
            self.draw_find_bar(display);
        }
        self.draw_status_bar(display);

        if let AppMode::FilePicker { .. } = &self.mode {
            self.draw_file_picker_overlay(display);
        }

        if self.menu.open.is_some() {
            self.draw_menu_popup(display);
        }
    }

    fn draw_title_bar(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let r = self.layout.title_bar;
        fill_rect(display, r, self.theme.colors.accent);
        // Close button
        let cb_w = 20i32;
        let cb_h = 18i32;
        let cb = Rectangle::new(
            Point::new(
                r.top_left.x + r.size.width as i32 - 3 - cb_w,
                r.top_left.y + (TITLE_H as i32 - cb_h) / 2,
            ),
            Size::new(cb_w as u32, cb_h as u32),
        );
        draw_title_button(display, &self.bevel, cb, false).ok();
        let cx = cb.top_left.x + cb.size.width as i32 / 2;
        let cy = cb.top_left.y + cb.size.height as i32 / 2;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        Line::new(Point::new(cx - 3, cy - 3), Point::new(cx + 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(display)
            .ok();
        Line::new(Point::new(cx + 3, cy - 3), Point::new(cx - 3, cy + 3))
            .into_styled(PrimitiveStyle::with_stroke(ink, 2))
            .draw(display)
            .ok();
        // Title text (mono, white — proportional title font would require more setup)
        let title = self.window_title();
        let style = MonoTextStyle::new(&FONT_6X10, self.theme.colors.caption_on_accent);
        let tx = r.top_left.x + 8;
        let ty = r.top_left.y + (TITLE_H as i32 - 10) / 2;
        Text::with_baseline(&title, Point::new(tx, ty), style, Baseline::Top)
            .draw(display)
            .ok();
    }

    fn draw_menu_bar(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let labels = &["File", "Edit", "View"];
        let cells = &self.layout.menu_cells;
        let focused = if self.menu.active {
            Some(self.menu.open.unwrap_or(0))
        } else {
            None
        };
        draw_menu_bar(
            display,
            self.layout.menu_bar,
            &cells[..],
            labels,
            focused,
            &FONT_6X10,
            self.theme.colors.surface,
            self.theme.colors.text,
            self.theme.colors.accent,
            self.theme.colors.caption_on_accent,
        )
        .ok();
    }

    fn draw_menu_popup(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let Some(midx) = self.menu.open else {
            return;
        };
        let items = menu_items(midx);
        let line_h = MENUBAR_H as i32 + 2;
        // Popup width: enough for the widest item label
        let popup_w = 200u32;
        let popup_h = (4 + items.len() as i32 * line_h + 4) as u32;
        let cell = self.layout.menu_cells[midx];
        let popup = Rectangle::new(
            Point::new(cell.top_left.x, cell.top_left.y + cell.size.height as i32),
            Size::new(popup_w, popup_h),
        );
        draw_menu_popup(
            display,
            &self.bevel,
            popup,
            items,
            self.menu.popup_sel,
            line_h,
            &FONT_6X10,
            self.theme.colors.canvas,
            self.theme.colors.text,
            self.theme.colors.selection_bg,
            self.theme.colors.caption_on_accent,
        )
        .ok();
    }

    fn draw_textarea(&mut self, display: &mut SimulatorDisplay<Rgb888>) {
        let area = self.layout.textarea;
        // White background
        fill_rect(display, area, Rgb888::WHITE);
        // Sunken border
        draw_sunken_field(display, &self.bevel, area, Rgb888::WHITE).ok();

        let inner = pad(area, BedrockBevel::BEVEL_PX);
        let line_h = self.layout.line_height;
        let lines = self.textarea.lines();
        let scroll = self.textarea.scroll_top_line;
        let vis = self.layout.visible_lines;
        let ink = Rgb888::new(0x0a, 0x0a, 0x0a);
        let sel_bg = Rgb888::new(0xc0, 0xc0, 0xc0);
        let sel_fg = ink;
        let find_hl = Rgb888::new(0xff, 0xff, 0x00); // yellow highlight for find matches

        // Find match byte ranges (clone to avoid borrow)
        let find_matches: Vec<(usize, usize)> = if self.find.visible {
            self.find.matches.clone()
        } else {
            Vec::new()
        };

        // Compute line byte offsets for selection and find highlight
        let mut line_byte_starts: Vec<usize> = Vec::with_capacity(lines.len());
        {
            let mut off = 0usize;
            for line in &lines {
                line_byte_starts.push(off);
                off += line.len() + 1; // +1 for '\n'
            }
        }

        for (row_offset, line_idx) in (scroll..(scroll + vis).min(lines.len())).enumerate() {
            let line = lines[line_idx];
            let line_start_byte = line_byte_starts.get(line_idx).copied().unwrap_or(0);

            let row_top = inner.top_left.y + EDITOR_PAD_TOP + row_offset as i32 * line_h;
            let baseline = row_top + (line_h as f32 * 0.78).round() as i32;
            // row_rect available for future selection-border use

            // Selection highlight
            if let Some((col_lo, col_hi)) = self.textarea.selection_highlight_on_line(line_idx) {
                let x0 =
                    inner.top_left.x + EDITOR_PAD_X + col_x_offset(line, col_lo, &mut self.cache);
                let x1 =
                    inner.top_left.x + EDITOR_PAD_X + col_x_offset(line, col_hi, &mut self.cache);
                let sel_rect = Rectangle::new(
                    Point::new(x0.max(inner.top_left.x), row_top),
                    Size::new((x1 - x0).max(0) as u32, line_h as u32),
                );
                fill_rect(display, sel_rect, sel_bg);
            }

            // Find match highlights on this line
            for &(mlo, mhi) in &find_matches {
                let line_end = line_start_byte + line.len();
                if mlo > line_end || mhi < line_start_byte {
                    continue;
                }
                let col_lo =
                    char_col_from_byte(line, mlo.saturating_sub(line_start_byte).min(line.len()));
                let col_hi =
                    char_col_from_byte(line, mhi.saturating_sub(line_start_byte).min(line.len()));
                let x0 =
                    inner.top_left.x + EDITOR_PAD_X + col_x_offset(line, col_lo, &mut self.cache);
                let x1 =
                    inner.top_left.x + EDITOR_PAD_X + col_x_offset(line, col_hi, &mut self.cache);
                let hl_rect = Rectangle::new(
                    Point::new(x0.max(inner.top_left.x), row_top),
                    Size::new((x1 - x0).max(1) as u32, line_h as u32),
                );
                fill_rect(display, hl_rect, find_hl);
            }

            // Draw text — determine per-char fg/bg based on selection
            let sel_range = self.textarea.selection_range_bytes();
            let mut cx = inner.top_left.x + EDITOR_PAD_X;
            for (ci, ch) in line.chars().enumerate() {
                let byte_pos = line_start_byte
                    + line
                        .char_indices()
                        .nth(ci)
                        .map(|(b, _)| b)
                        .unwrap_or(line.len());
                let in_sel = sel_range
                    .map(|(lo, hi)| byte_pos >= lo && byte_pos < hi)
                    .unwrap_or(false);
                let bg = if in_sel { sel_bg } else { Rgb888::WHITE };
                let fg = if in_sel { sel_fg } else { ink };
                cx = draw_glyph_line(
                    display,
                    &mut self.cache,
                    &ch.to_string(),
                    cx,
                    baseline,
                    fg,
                    bg,
                    inner,
                );
            }

            // Draw cursor (blinking handled by always drawing for now)
            let cursor_byte = self.textarea.cursor();
            let cursor_line_idx = line_index_for_cursor(&self.textarea, &lines, &line_byte_starts);
            if cursor_line_idx == line_idx {
                let col = char_col_from_byte(
                    line,
                    cursor_byte.saturating_sub(line_start_byte).min(line.len()),
                );
                let cursor_x =
                    inner.top_left.x + EDITOR_PAD_X + col_x_offset(line, col, &mut self.cache);
                Line::new(
                    Point::new(cursor_x, row_top + 1),
                    Point::new(cursor_x, row_top + line_h - 2),
                )
                .into_styled(PrimitiveStyle::with_stroke(ink, 1))
                .draw(display)
                .ok();
            }
        }
    }

    fn draw_scrollbar(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let total = self.textarea.line_count().max(1);
        let vis = self.layout.visible_lines.max(1);
        let scroll = self.textarea.scroll_top_line;
        let sb = ScrollbarState::new(ScrollAxis::Vertical, total, vis, scroll);
        draw_scrollbar_vertical(
            display,
            &self.bevel,
            self.layout.scrollbar,
            sb.thumb_center_ratio(),
            sb.thumb_length_ratio(),
            false,
            false,
        )
        .ok();
    }

    fn draw_find_bar(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let r = self.layout.find_bar;
        fill_rect(display, r, self.theme.colors.surface);
        draw_status_border(display, &self.bevel, r).ok();
        let style = MonoTextStyle::new(&FONT_6X10, self.theme.colors.text);
        let lbl_y = r.top_left.y + (FINDBAR_H as i32 - 10) / 2;
        Text::with_baseline(
            "Find:",
            Point::new(r.top_left.x + 6, lbl_y),
            style,
            Baseline::Top,
        )
        .draw(display)
        .ok();
        // Input field
        let field_x = r.top_left.x + 44;
        let field_w = 240u32;
        let field = Rectangle::new(
            Point::new(field_x, r.top_left.y + 4),
            Size::new(field_w, FINDBAR_H - 8),
        );
        draw_sunken_field(display, &self.bevel, field, self.theme.colors.canvas).ok();
        let fi = pad(field, BedrockBevel::BEVEL_PX);
        let text_y = fi.top_left.y + (fi.size.height as i32 - 10) / 2;
        Text::with_baseline(
            &self.find.query.text,
            Point::new(fi.top_left.x + 2, text_y),
            style,
            Baseline::Top,
        )
        .draw(display)
        .ok();
        // Match count
        let count_x = field_x + field_w as i32 + 10;
        let count_str = if self.find.query.text.is_empty() {
            String::new()
        } else if self.find.matches.is_empty() {
            "No matches".to_string()
        } else {
            format!("{}/{}", self.find.current + 1, self.find.matches.len())
        };
        Text::with_baseline(&count_str, Point::new(count_x, lbl_y), style, Baseline::Top)
            .draw(display)
            .ok();
    }

    fn draw_status_bar(&self, display: &mut SimulatorDisplay<Rgb888>) {
        let r = self.layout.status_bar;
        fill_rect(display, r, self.theme.colors.surface);
        draw_status_border(display, &self.bevel, r).ok();
        let (ln, col) = cursor_line_col(&self.textarea);
        let name = self.filepath.as_deref().unwrap_or("Untitled");
        let font_px = FONT_SIZES[self.font_size_idx] as u32;
        let status = format!(
            "Ln {}, Col {}     {}     Font: {}px",
            ln + 1,
            col + 1,
            name,
            font_px
        );
        let style = MonoTextStyle::new(&FONT_6X10, self.theme.colors.text);
        let ty = r.top_left.y + (STATUSBAR_H as i32 - 10) / 2;
        Text::with_baseline(
            &status,
            Point::new(r.top_left.x + 8, ty),
            style,
            Baseline::Top,
        )
        .draw(display)
        .ok();
    }

    fn draw_file_picker_overlay(&mut self, display: &mut SimulatorDisplay<Rgb888>) {
        let AppMode::FilePicker { ref state, .. } = self.mode else {
            return;
        };

        let dw = (self.win_w * 4 / 5).min(560);
        let dh = (self.win_h * 4 / 5).min(380);
        let dx = (self.win_w - dw) / 2;
        let dy = (self.win_h - dh) / 2;
        let dialog = Rectangle::new(Point::new(dx as i32, dy as i32), Size::new(dw, dh));

        // Dim background
        fill_rect(
            display,
            Rectangle::new(Point::zero(), display.size()),
            Rgb888::new(0x40, 0x40, 0x40),
        );

        let lh = 18i32;
        let layout = compute_file_picker_layout(dialog, lh, FP_SB_W);
        let scroll = ScrollbarState::new(
            ScrollAxis::Vertical,
            state.picker.entries.len().max(1),
            layout.visible_rows.max(1),
            state.picker.scroll_top,
        );
        let ft_labels = &["All files (*.*)", "Text files (*.txt)"];
        draw_file_picker(
            display,
            &self.bevel,
            &layout,
            &state.picker,
            &state.filename.text,
            ft_labels,
            state.filetype_sel,
            matches!(
                self.mode,
                AppMode::FilePicker {
                    reason: FilePickerReason::Open,
                    ..
                }
            ),
            state.focus,
            lh,
            6,
            &FONT_6X10,
            &self.theme.colors,
            &scroll,
        )
        .ok();
    }

    // ── event handling ─────────────────────────────────────────────────────────

    fn handle_event(&mut self, ev: SimulatorEvent) -> bool {
        match ev {
            SimulatorEvent::Quit => {
                // Quit fires for both window-close and Escape.
                // Check contextual Escape first.
                if self.handle_escape() {
                    return false; // consumed, don't quit
                }
                // Real quit — check dirty
                if self.dirty {
                    // For simplicity: just quit without save prompt in this prototype
                    // A real app would show a confirm dialog here
                }
                return true; // quit
            }
            SimulatorEvent::KeyDown {
                keycode,
                keymod,
                repeat,
            } => {
                let _ = repeat;
                let mods = sdl_mod_to_modifiers(keymod);
                self.handle_keydown(keycode, mods);
            }
            SimulatorEvent::MouseButtonDown { point, .. } => {
                self.handle_click(point);
            }
            SimulatorEvent::MouseWheel { scroll_delta, .. } => {
                // Scroll textarea (scroll_delta.y positive = up on most platforms)
                if matches!(self.mode, AppMode::Editing) {
                    let lines = self.textarea.line_count();
                    let vis = self.layout.visible_lines;
                    if scroll_delta.y < 0 {
                        self.textarea.scroll_top_line = self
                            .textarea
                            .scroll_top_line
                            .saturating_add(3)
                            .min(lines.saturating_sub(vis));
                    } else {
                        self.textarea.scroll_top_line =
                            self.textarea.scroll_top_line.saturating_sub(3);
                    }
                }
            }
            _ => {}
        }
        false
    }

    fn handle_escape(&mut self) -> bool {
        if self.menu.open.is_some() {
            self.menu.open = None;
            self.menu.active = true; // keep bar focus so user can navigate
            return true;
        }
        if self.menu.active {
            self.menu.active = false;
            return true;
        }
        if self.find.visible {
            self.find.visible = false;
            self.rebuild_layout();
            return true;
        }
        if matches!(self.mode, AppMode::FilePicker { .. }) {
            self.mode = AppMode::Editing;
            return true;
        }
        false
    }

    fn handle_keydown(&mut self, kc: Keycode, mods: Modifiers) {
        // F11: toggle fullscreen
        if kc == Keycode::F11 {
            self.fullscreen = !self.fullscreen;
            // Window resize is handled externally; for now just note the toggle
            return;
        }

        // File picker mode: route all keys there
        if matches!(self.mode, AppMode::FilePicker { .. }) {
            self.handle_picker_key(kc, mods);
            return;
        }

        // Menu active: route to menu
        if self.menu.active || self.menu.open.is_some() {
            self.handle_menu_key(kc, mods);
            return;
        }

        // F10 / Alt: open menu bar
        if kc == Keycode::F10 || (mods.alt && !mods.ctrl) {
            self.menu.active = true;
            self.menu.open = None;
            return;
        }

        // Find bar active: Ctrl+F or Enter/F3 to cycle
        if self.find.visible {
            match kc {
                Keycode::Return | Keycode::F3 => {
                    self.find_next();
                    return;
                }
                _ => {
                    // Route character input to find query
                    if let Some(key) = sdl_to_key(kc, mods) {
                        let changed = self.find.query.apply_key(key);
                        if changed {
                            let text = self.textarea.text.clone();
                            self.find.search(&text);
                        }
                        return;
                    }
                }
            }
        }

        // Editing shortcuts
        if self.handle_editing_shortcut(kc, mods) {
            return;
        }

        // Pass to textarea
        if let Some(key) = sdl_to_key(kc, mods) {
            let ev = KeyEvent::with_modifiers(key, mods);
            self.textarea.apply_key_event(&ev);
            self.dirty = true;
            let vis = self.layout.visible_lines;
            self.textarea.scroll_to_cursor(vis);
        }
    }

    fn handle_editing_shortcut(&mut self, kc: Keycode, mods: Modifiers) -> bool {
        if mods.ctrl {
            match kc {
                Keycode::N => {
                    self.cmd_new();
                    return true;
                }
                Keycode::O => {
                    self.cmd_open();
                    return true;
                }
                Keycode::S if mods.shift => {
                    self.cmd_save_as();
                    return true;
                }
                Keycode::S => {
                    self.cmd_save();
                    return true;
                }
                Keycode::X => {
                    self.cmd_cut();
                    return true;
                }
                Keycode::C => {
                    self.cmd_copy();
                    return true;
                }
                Keycode::V => {
                    self.cmd_paste();
                    return true;
                }
                Keycode::A => {
                    self.textarea.select_all();
                    return true;
                }
                Keycode::F => {
                    self.cmd_find();
                    return true;
                }
                Keycode::Equals | Keycode::Plus => {
                    self.font_size_change(1);
                    return true;
                }
                Keycode::Minus => {
                    self.font_size_change(-1);
                    return true;
                }
                _ => {}
            }
        }
        // Delete key without Ctrl: handled by textarea, but if no selection just forward
        false
    }

    fn handle_menu_key(&mut self, kc: Keycode, _mods: Modifiers) {
        if let Some(midx) = self.menu.open {
            // Popup is open
            let items = menu_items(midx);
            match kc {
                Keycode::Up => {
                    if self.menu.popup_sel > 0 {
                        self.menu.popup_sel -= 1;
                    }
                    // Skip separators
                    while items.get(self.menu.popup_sel) == Some(&"—") && self.menu.popup_sel > 0
                    {
                        self.menu.popup_sel -= 1;
                    }
                }
                Keycode::Down => {
                    if self.menu.popup_sel + 1 < items.len() {
                        self.menu.popup_sel += 1;
                    }
                    while items.get(self.menu.popup_sel) == Some(&"—")
                        && self.menu.popup_sel + 1 < items.len()
                    {
                        self.menu.popup_sel += 1;
                    }
                }
                Keycode::Left => {
                    self.menu.open = Some(if midx == 0 { 2 } else { midx - 1 });
                    self.menu.popup_sel = 0;
                }
                Keycode::Right => {
                    self.menu.open = Some((midx + 1) % 3);
                    self.menu.popup_sel = 0;
                }
                Keycode::Return => {
                    self.activate_menu_item(midx, self.menu.popup_sel);
                    self.menu.open = None;
                    self.menu.active = false;
                }
                _ => {
                    self.menu.open = None;
                    self.menu.active = false;
                }
            }
        } else {
            // Bar focused, no popup
            match kc {
                Keycode::Left => {}
                Keycode::Right => {}
                Keycode::Return | Keycode::Down => {
                    self.menu.open = Some(0);
                    self.menu.popup_sel = 0;
                }
                _ => {
                    self.menu.active = false;
                }
            }
        }
    }

    fn activate_menu_item(&mut self, menu_idx: usize, item_idx: usize) {
        match (menu_idx, item_idx) {
            // File menu
            (0, 0) => self.cmd_new(),
            (0, 1) => self.cmd_open(),
            (0, 2) => self.cmd_save(),
            (0, 3) => self.cmd_save_as(),
            // (0, 4) is separator
            // (0, 5) exit — handled by returning true from handle_event
            // Edit menu
            (1, 0) => self.cmd_cut(),
            (1, 1) => self.cmd_copy(),
            (1, 2) => self.cmd_paste(),
            (1, 3) => {
                self.textarea.apply_key(Key::Delete);
                self.dirty = true;
            }
            // (1, 4) separator
            (1, 5) => self.cmd_find(),
            (1, 6) => {
                self.textarea.select_all();
            }
            // View menu
            (2, 0) => self.font_size_change(1),
            (2, 1) => self.font_size_change(-1),
            _ => {}
        }
    }

    fn handle_picker_key(&mut self, kc: Keycode, mods: Modifiers) {
        let Some(key) = sdl_to_key(kc, mods) else {
            return;
        };
        let ev = KeyEvent::with_modifiers(key, mods);

        // Extract state temporarily to avoid borrow conflict with self.std_io
        let mode = std::mem::replace(&mut self.mode, AppMode::Editing);
        if let AppMode::FilePicker { reason, mut state } = mode {
            let visible_rows = {
                let dw = (self.win_w * 4 / 5).min(560);
                let dh = (self.win_h * 4 / 5).min(380);
                let dialog = Rectangle::new(Point::zero(), Size::new(dw, dh));
                let lyt = compute_file_picker_layout(dialog, 18, FP_SB_W);
                lyt.visible_rows
            };
            match state.handle_key(&ev, visible_rows, &mut self.std_io) {
                Ok(FilePickerDialogAction::Confirm { path, name }) => {
                    match reason {
                        FilePickerReason::Open => self.load_file(path, name),
                        FilePickerReason::SaveAs => self.save_file_as(path, name),
                    }
                    self.mode = AppMode::Editing;
                }
                Ok(FilePickerDialogAction::Cancel) => {
                    self.mode = AppMode::Editing;
                }
                Ok(FilePickerDialogAction::None) => {
                    self.mode = AppMode::FilePicker { reason, state };
                }
                Err(_) => {
                    self.mode = AppMode::Editing;
                }
            }
        }
    }

    fn handle_click(&mut self, point: Point) {
        // Click on menu bar cells
        for (i, cell) in self.layout.menu_cells.iter().enumerate() {
            if cell.contains(point) {
                if self.menu.open == Some(i) {
                    self.menu.open = None;
                    self.menu.active = false;
                } else {
                    self.menu.open = Some(i);
                    self.menu.popup_sel = 0;
                    self.menu.active = true;
                }
                return;
            }
        }
        // Close menu on click outside
        if self.menu.open.is_some() {
            self.menu.open = None;
            self.menu.active = false;
            return;
        }
        // Click in textarea: move cursor
        if self.layout.textarea.contains(point) {
            let inner = pad(self.layout.textarea, BedrockBevel::BEVEL_PX);
            let lh = self.layout.line_height;
            let row = ((point.y - inner.top_left.y - EDITOR_PAD_TOP) / lh) as usize;
            let line_idx = (self.textarea.scroll_top_line + row)
                .min(self.textarea.line_count().saturating_sub(1));
            let lines = self.textarea.lines();
            if let Some(line) = lines.get(line_idx) {
                let lx = point.x - inner.top_left.x - EDITOR_PAD_X;
                let col = col_from_x(line, lx, &mut self.cache);
                // Compute byte offset
                let line_start: usize = lines[..line_idx].iter().map(|l| l.len() + 1).sum();
                let byte_off = line_start + char_byte_offset(line, col);
                self.textarea.set_cursor(byte_off);
            }
        }
    }

    // ── commands ───────────────────────────────────────────────────────────────

    fn cmd_new(&mut self) {
        self.textarea = TextArea::new();
        self.filepath = None;
        self.dirty = false;
        self.rebuild_layout();
    }

    fn cmd_open(&mut self) {
        match FilePickerDialogState::new(PickerMode::Load, 2, &mut self.std_io) {
            Ok(state) => {
                self.mode = AppMode::FilePicker {
                    reason: FilePickerReason::Open,
                    state: Box::new(state),
                };
            }
            Err(_) => {}
        }
    }

    fn cmd_save(&mut self) {
        if let Some(ref path) = self.filepath.clone() {
            let data = self.textarea.text.as_bytes().to_vec();
            if std::fs::write(path, data).is_ok() {
                self.dirty = false;
            }
        } else {
            self.cmd_save_as();
        }
    }

    fn cmd_save_as(&mut self) {
        match FilePickerDialogState::new(PickerMode::Save, 2, &mut self.std_io) {
            Ok(state) => {
                self.mode = AppMode::FilePicker {
                    reason: FilePickerReason::SaveAs,
                    state: Box::new(state),
                };
            }
            Err(_) => {}
        }
    }

    fn cmd_cut(&mut self) {
        if let Some(text) = self.textarea.selected_text() {
            self.clipboard = text;
            self.textarea.replace_selection_with_str("");
            self.dirty = true;
        }
    }

    fn cmd_copy(&mut self) {
        if let Some(text) = self.textarea.selected_text() {
            self.clipboard = text;
        }
    }

    fn cmd_paste(&mut self) {
        let cb = self.clipboard.clone();
        self.textarea.replace_selection_with_str(&cb);
        self.dirty = true;
        let vis = self.layout.visible_lines;
        self.textarea.scroll_to_cursor(vis);
    }

    fn cmd_find(&mut self) {
        self.find.visible = true;
        self.rebuild_layout();
        let text = self.textarea.text.clone();
        self.find.search(&text);
    }

    fn find_next(&mut self) {
        if self.find.matches.is_empty() {
            return;
        }
        self.find.current = (self.find.current + 1) % self.find.matches.len();
        let (lo, _hi) = self.find.matches[self.find.current];
        self.textarea.set_cursor(lo);
        let vis = self.layout.visible_lines;
        self.textarea.scroll_to_cursor(vis);
    }

    fn font_size_change(&mut self, delta: i32) {
        let new_idx =
            (self.font_size_idx as i32 + delta).clamp(0, FONT_SIZES.len() as i32 - 1) as usize;
        if new_idx != self.font_size_idx {
            self.font_size_idx = new_idx;
            self.cache.set_size(FONT_SIZES[new_idx]);
            self.rebuild_layout();
        }
    }

    fn load_file(&mut self, path: Vec<String>, name: String) {
        match self.std_io.read_file(&path, &name) {
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes).into_owned();
                self.textarea = TextArea::from_str(&text);
                let mut full_path = path_from_parts(&path);
                full_path.push(&name);
                self.filepath = Some(full_path.to_string_lossy().into_owned());
                self.dirty = false;
                self.rebuild_layout();
            }
            Err(e) => eprintln!("load_file error: {e}"),
        }
    }

    fn save_file_as(&mut self, path: Vec<String>, name: String) {
        let data = self.textarea.text.as_bytes().to_vec();
        match self.std_io.write_file(&path, &name, &data) {
            Ok(()) => {
                let mut full_path = path_from_parts(&path);
                full_path.push(&name);
                self.filepath = Some(full_path.to_string_lossy().into_owned());
                self.dirty = false;
            }
            Err(e) => eprintln!("save_as error: {e}"),
        }
    }
}

// ── layout computation ────────────────────────────────────────────────────────

fn build_layout(w: u32, h: u32, line_h: i32, find_visible: bool) -> EditorLayout {
    let x = 0i32;
    let y = 0i32;
    let title_bar = Rectangle::new(Point::new(x, y), Size::new(w, TITLE_H));
    let menu_bar = Rectangle::new(Point::new(x, y + TITLE_H as i32), Size::new(w, MENUBAR_H));

    // Menu cells (File / Edit / View)
    let cell_labels = [("File", 50u32), ("Edit", 50u32), ("View", 50u32)];
    let mut cx = x + 4;
    let cell_y = y + TITLE_H as i32;
    let mut menu_cells = [Rectangle::zero(); 3];
    for (i, (_lbl, cw)) in cell_labels.iter().enumerate() {
        menu_cells[i] = Rectangle::new(Point::new(cx, cell_y), Size::new(*cw, MENUBAR_H));
        cx += *cw as i32 + 2;
    }

    let content_top = TITLE_H as i32 + MENUBAR_H as i32;
    let content_bot = h as i32 - STATUSBAR_H as i32;
    let ta_h = if find_visible {
        (content_bot - content_top - FINDBAR_H as i32).max(0) as u32
    } else {
        (content_bot - content_top).max(0) as u32
    };

    let textarea = Rectangle::new(Point::new(x, content_top), Size::new(w - SB_W, ta_h));
    let scrollbar = Rectangle::new(
        Point::new(x + (w - SB_W) as i32, content_top),
        Size::new(SB_W, ta_h),
    );

    let find_bar = Rectangle::new(
        Point::new(x, content_top + ta_h as i32),
        Size::new(w, FINDBAR_H),
    );
    let status_bar = Rectangle::new(Point::new(x, content_bot), Size::new(w, STATUSBAR_H));

    let inner_h = ta_h.saturating_sub(BedrockBevel::BEVEL_PX as u32 * 2 + EDITOR_PAD_TOP as u32);
    let visible_lines = if line_h > 0 {
        inner_h as usize / line_h as usize
    } else {
        1
    };

    EditorLayout {
        title_bar,
        menu_bar,
        menu_cells,
        textarea,
        scrollbar,
        find_bar,
        status_bar,
        line_height: line_h,
        visible_lines,
    }
}

// ── utility functions ─────────────────────────────────────────────────────────

fn fill_rect(display: &mut SimulatorDisplay<Rgb888>, r: Rectangle, color: Rgb888) {
    r.into_styled(PrimitiveStyleBuilder::new().fill_color(color).build())
        .draw(display)
        .ok();
}

fn cursor_line_col(ta: &TextArea) -> (usize, usize) {
    let cursor = ta.cursor();
    let lines = ta.lines();
    let mut off = 0usize;
    for (i, line) in lines.iter().enumerate() {
        let end = off + line.len();
        if cursor <= end {
            let col = ta.text[off..cursor.min(end)].chars().count();
            return (i, col);
        }
        off = end + 1; // +1 for '\n'
    }
    (lines.len().saturating_sub(1), 0)
}

fn line_index_for_cursor(ta: &TextArea, lines: &[&str], line_starts: &[usize]) -> usize {
    let cursor = ta.cursor();
    for (i, &start) in line_starts.iter().enumerate() {
        let end = start + lines.get(i).map(|l| l.len()).unwrap_or(0);
        if cursor <= end {
            return i;
        }
    }
    lines.len().saturating_sub(1)
}

/// Pixel x-offset of column `col` (char index) within `line`.
fn col_x_offset(line: &str, col: usize, cache: &mut GlyphCache) -> i32 {
    line.chars().take(col).map(|c| cache.char_advance(c)).sum()
}

/// Character column from byte offset within a line.
fn char_col_from_byte(line: &str, byte: usize) -> usize {
    let byte = byte.min(line.len());
    line[..byte].chars().count()
}

/// Byte offset of character at column `col` within `line`.
fn char_byte_offset(line: &str, col: usize) -> usize {
    line.char_indices()
        .nth(col)
        .map(|(b, _)| b)
        .unwrap_or(line.len())
}

/// Find the character column closest to pixel x in `line`.
fn col_from_x(line: &str, target_x: i32, cache: &mut GlyphCache) -> usize {
    let mut x = 0i32;
    for (col, ch) in line.chars().enumerate() {
        let adv = cache.char_advance(ch);
        if x + adv / 2 >= target_x {
            return col;
        }
        x += adv;
    }
    line.chars().count()
}

fn sdl_mod_to_modifiers(m: Mod) -> Modifiers {
    Modifiers {
        shift: m.intersects(Mod::LSHIFTMOD | Mod::RSHIFTMOD),
        ctrl: m.intersects(Mod::LCTRLMOD | Mod::RCTRLMOD),
        alt: m.intersects(Mod::LALTMOD | Mod::RALTMOD),
    }
}

fn sdl_to_key(kc: Keycode, mods: Modifiers) -> Option<Key> {
    Some(match kc {
        Keycode::Left => Key::Left,
        Keycode::Right => Key::Right,
        Keycode::Up => Key::Up,
        Keycode::Down => Key::Down,
        Keycode::Return => Key::Enter,
        Keycode::Tab => Key::Tab,
        Keycode::Backspace => Key::Backspace,
        Keycode::Delete => Key::Delete,
        Keycode::Home => Key::Home,
        Keycode::End => Key::End,
        Keycode::PageUp => Key::PageUp,
        Keycode::PageDown => Key::PageDown,
        Keycode::F1 => Key::Function(1),
        Keycode::F2 => Key::Function(2),
        Keycode::F3 => Key::Function(3),
        Keycode::F4 => Key::Function(4),
        Keycode::F5 => Key::Function(5),
        _ => {
            // Map printable keycodes to characters
            let c = sdl_keycode_to_char(kc, mods)?;
            Key::Character(c)
        }
    })
}

fn sdl_keycode_to_char(kc: Keycode, mods: Modifiers) -> Option<char> {
    let shift = mods.shift;
    Some(match kc {
        Keycode::Space => ' ',
        Keycode::Exclaim => '!',
        Keycode::Quotedbl => '"',
        Keycode::Hash => '#',
        Keycode::Dollar => '$',
        Keycode::Percent => '%',
        Keycode::Ampersand => '&',
        Keycode::Quote => {
            if shift {
                '"'
            } else {
                '\''
            }
        }
        Keycode::LeftParen => '(',
        Keycode::RightParen => ')',
        Keycode::Asterisk => '*',
        Keycode::Plus => '+',
        Keycode::Comma => {
            if shift {
                '<'
            } else {
                ','
            }
        }
        Keycode::Minus => {
            if shift {
                '_'
            } else {
                '-'
            }
        }
        Keycode::Period => {
            if shift {
                '>'
            } else {
                '.'
            }
        }
        Keycode::Slash => {
            if shift {
                '?'
            } else {
                '/'
            }
        }
        Keycode::Num0 => {
            if shift {
                ')'
            } else {
                '0'
            }
        }
        Keycode::Num1 => {
            if shift {
                '!'
            } else {
                '1'
            }
        }
        Keycode::Num2 => {
            if shift {
                '@'
            } else {
                '2'
            }
        }
        Keycode::Num3 => {
            if shift {
                '#'
            } else {
                '3'
            }
        }
        Keycode::Num4 => {
            if shift {
                '$'
            } else {
                '4'
            }
        }
        Keycode::Num5 => {
            if shift {
                '%'
            } else {
                '5'
            }
        }
        Keycode::Num6 => {
            if shift {
                '^'
            } else {
                '6'
            }
        }
        Keycode::Num7 => {
            if shift {
                '&'
            } else {
                '7'
            }
        }
        Keycode::Num8 => {
            if shift {
                '*'
            } else {
                '8'
            }
        }
        Keycode::Num9 => {
            if shift {
                '('
            } else {
                '9'
            }
        }
        Keycode::Colon => ':',
        Keycode::Semicolon => {
            if shift {
                ':'
            } else {
                ';'
            }
        }
        Keycode::Less => '<',
        Keycode::Equals => {
            if shift {
                '+'
            } else {
                '='
            }
        }
        Keycode::Greater => '>',
        Keycode::Question => '?',
        Keycode::At => '@',
        Keycode::LeftBracket => {
            if shift {
                '{'
            } else {
                '['
            }
        }
        Keycode::Backslash => {
            if shift {
                '|'
            } else {
                '\\'
            }
        }
        Keycode::RightBracket => {
            if shift {
                '}'
            } else {
                ']'
            }
        }
        Keycode::Caret => '^',
        Keycode::Underscore => '_',
        Keycode::Backquote => {
            if shift {
                '~'
            } else {
                '`'
            }
        }
        Keycode::A => {
            if shift {
                'A'
            } else {
                'a'
            }
        }
        Keycode::B => {
            if shift {
                'B'
            } else {
                'b'
            }
        }
        Keycode::C => {
            if shift {
                'C'
            } else {
                'c'
            }
        }
        Keycode::D => {
            if shift {
                'D'
            } else {
                'd'
            }
        }
        Keycode::E => {
            if shift {
                'E'
            } else {
                'e'
            }
        }
        Keycode::F => {
            if shift {
                'F'
            } else {
                'f'
            }
        }
        Keycode::G => {
            if shift {
                'G'
            } else {
                'g'
            }
        }
        Keycode::H => {
            if shift {
                'H'
            } else {
                'h'
            }
        }
        Keycode::I => {
            if shift {
                'I'
            } else {
                'i'
            }
        }
        Keycode::J => {
            if shift {
                'J'
            } else {
                'j'
            }
        }
        Keycode::K => {
            if shift {
                'K'
            } else {
                'k'
            }
        }
        Keycode::L => {
            if shift {
                'L'
            } else {
                'l'
            }
        }
        Keycode::M => {
            if shift {
                'M'
            } else {
                'm'
            }
        }
        Keycode::N => {
            if shift {
                'N'
            } else {
                'n'
            }
        }
        Keycode::O => {
            if shift {
                'O'
            } else {
                'o'
            }
        }
        Keycode::P => {
            if shift {
                'P'
            } else {
                'p'
            }
        }
        Keycode::Q => {
            if shift {
                'Q'
            } else {
                'q'
            }
        }
        Keycode::R => {
            if shift {
                'R'
            } else {
                'r'
            }
        }
        Keycode::S => {
            if shift {
                'S'
            } else {
                's'
            }
        }
        Keycode::T => {
            if shift {
                'T'
            } else {
                't'
            }
        }
        Keycode::U => {
            if shift {
                'U'
            } else {
                'u'
            }
        }
        Keycode::V => {
            if shift {
                'V'
            } else {
                'v'
            }
        }
        Keycode::W => {
            if shift {
                'W'
            } else {
                'w'
            }
        }
        Keycode::X => {
            if shift {
                'X'
            } else {
                'x'
            }
        }
        Keycode::Y => {
            if shift {
                'Y'
            } else {
                'y'
            }
        }
        Keycode::Z => {
            if shift {
                'Z'
            } else {
                'z'
            }
        }
        Keycode::KpEnter => '\n',
        Keycode::KpPlus => '+',
        Keycode::KpMinus => '-',
        Keycode::KpMultiply => '*',
        Keycode::KpDivide => '/',
        Keycode::KpPeriod => '.',
        Keycode::Kp0 => '0',
        Keycode::Kp1 => '1',
        Keycode::Kp2 => '2',
        Keycode::Kp3 => '3',
        Keycode::Kp4 => '4',
        Keycode::Kp5 => '5',
        Keycode::Kp6 => '6',
        Keycode::Kp7 => '7',
        Keycode::Kp8 => '8',
        Keycode::Kp9 => '9',
        _ => return None,
    })
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    // Load Tinos-Regular font
    let font_bytes = match std::fs::read(TINOS_PATH) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Cannot load font at {TINOS_PATH}: {e}");
            eprintln!("Please ensure Tinos-Regular.ttf is at that path.");
            std::process::exit(1);
        }
    };
    let font = fontdue::Font::from_bytes(font_bytes.as_slice(), fontdue::FontSettings::default())
        .expect("Failed to parse font");

    let mut app = EditorApp::new(font);

    // If a file path is given as argument, open it
    let args: Vec<String> = std::env::args().collect();
    if let Some(path) = args.get(1) {
        let p = std::path::Path::new(path);
        if p.exists() {
            match std::fs::read(p) {
                Ok(bytes) => {
                    app.textarea = TextArea::from_str(&String::from_utf8_lossy(&bytes));
                    app.filepath = Some(path.clone());
                    app.dirty = false;
                }
                Err(e) => eprintln!("Could not open {path}: {e}"),
            }
        }
    }

    let output_settings = OutputSettingsBuilder::new().scale(1).build();
    let mut window = Window::new(&app.window_title(), &output_settings);
    let mut display: SimulatorDisplay<Rgb888> = SimulatorDisplay::new(Size::new(WIN_W, WIN_H));

    'running: loop {
        // Draw frame
        app.draw(&mut display);
        window.update(&display);

        // Process events
        for event in window.events() {
            if app.handle_event(event) {
                break 'running;
            }
        }
    }
}
