//! UEFI interactive demo: Bedrock-style window, menu bar, textarea, pointers (absolute + simple).
//! Embeds **EB Garamond** (`.ttf`) for the title and a caption, and **`assets/images/test.png`**
//! decoded with [`uefi_ui::png`] for a preview strip (see `assets/fonts/README.txt`).
#![no_main]
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use core::time::Duration;

use uefi::boot::{
    self, find_handles, open_protocol_exclusive, ScopedProtocol, EventType, TimerTrigger, Tpl,
};
use uefi::prelude::*;
use uefi::Event;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::console::pointer::Pointer;
use uefi::proto::console::text::{Input, Key as UefiKey, ScanCode};
use uefi::proto::media::fs::SimpleFileSystem;

use uefi_ui::embedded_graphics::pixelcolor::Rgb888;
use uefi_ui::embedded_graphics::mono_font::ascii::FONT_6X10;
use uefi_ui::embedded_graphics::mono_font::MonoTextStyle;
use uefi_ui::embedded_graphics::mono_font::MonoFont;
use uefi_ui::embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use uefi_ui::embedded_graphics::text::{Baseline, Text};
use uefi_ui::embedded_graphics::prelude::*;

use uefi_ui::file_picker::{FileIo, FilePickerDialogAction, FilePickerDialogState, PickerMode};
use uefi_ui::font::load_font;
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::input::{Key, KeyEvent, Modifiers};
use uefi_ui::pointer::{index_at, PointerState};
use uefi_ui::popover::{PopoverKind, PopoverSpec, PopoverStack};
use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::bedrock_controls::{compute_file_picker_layout, draw_file_picker, draw_menu_popup, FP_SB_W, MENU_POPUP_TOP_PAD};
use uefi_ui::theme::Theme;
use uefi_ui::{SimpleFsIo, list_simple_fs_handles};
use uefi_ui::widgets::{
    textarea_sync_vertical_scroll, NavBar, ScrollAxis, ScrollbarState, TextArea,
};
use uefi_ui::window::{WindowOffset, WindowStack};

mod absolute_pointer;

use absolute_pointer::{abs_state_key, map_abs_to_pixels, primary_button, right_button, AbsolutePointer};
use uefi_ui_test::demo_gallery::{
    apply_gallery_key_event, gallery_pointer_down, GalleryState,
};
use uefi_ui_test::layout::{
    compute_layout, submenu_popup_rect, Focus, UiLayout, MENU_LABELS, SUBMENUS,
};
use uefi_ui_test::tab_order::apply_tab;
use uefi_ui_test::scene::{decode_demo_png, paint_scene_no_cursor, DEMO_PNG_BYTES};

const ABOUT_MODAL_ID: u64 = 1;

/// Context menu items shown on right-click in the text area.
/// `"—"` renders as a horizontal separator.
static CONTEXT_ITEMS: &[&str] = &["&Cut", "&Copy", "&Paste", "—", "Select &All"];

/// Actions corresponding to each non-separator item in [`CONTEXT_ITEMS`].
#[derive(Clone, Copy, PartialEq, Eq)]
enum ContextAction { Cut, Copy, Paste, SelectAll }

static CONTEXT_ACTIONS: &[Option<ContextAction>] = &[
    Some(ContextAction::Cut),
    Some(ContextAction::Copy),
    Some(ContextAction::Paste),
    None, // separator
    Some(ContextAction::SelectAll),
];

/// Live context-menu state: which popup rect, which item is highlighted.
struct ContextMenuState {
    rect: uefi_ui::embedded_graphics::primitives::Rectangle,
    hovered: usize,
}

/// Drag affordance state for file-picker list items (T-17).
struct FpDrag {
    label: String,
    x: i32,
    y: i32,
}

/// [EB Garamond](https://fonts.google.com/specimen/EB+Garamond) variable font (OFL); see `assets/fonts/`.
static EB_GARAMOND_TTF: &[u8] = include_bytes!("../../../assets/fonts/EBGaramond-VF.ttf");
/// Sample PNG decoded at boot with `minipng` (moved from repo root → `assets/images/`).

/// Force OVMF to bind drivers to all discovered controllers (USB, PCI, etc).
/// Without this, USB pointing devices may not have protocol handlers installed.
fn connect_all_controllers() {
    // Connect every handle in the system recursively — this triggers OVMF's USB
    // enumeration, binding the USB HID driver to the USB tablet/mouse.
    if let Ok(handles) = boot::locate_handle_buffer(uefi::boot::SearchType::AllHandles) {
        for &h in handles.iter() {
            let _ = boot::connect_controller(h, None, None, true);
        }
    }
}

fn collect_absolute_pointers() -> Vec<ScopedProtocol<AbsolutePointer>> {
    let mut v = Vec::new();
    if let Ok(handles) = find_handles::<AbsolutePointer>() {
        uefi::println!("  find_handles::<AbsolutePointer> => {} handles", handles.len());
        for h in handles {
            if let Ok(mut p) = open_protocol_exclusive::<AbsolutePointer>(h) {
                let _ = p.reset(false);
                v.push(p);
            }
        }
    }
    if v.is_empty() {
        if let Ok(h) = boot::get_handle_for_protocol::<AbsolutePointer>() {
            if let Ok(mut p) = open_protocol_exclusive::<AbsolutePointer>(h) {
                let _ = p.reset(false);
                v.push(p);
            }
        }
    }
    v
}

fn collect_simple_pointers() -> Vec<ScopedProtocol<Pointer>> {
    let mut v = Vec::new();
    if let Ok(handles) = find_handles::<Pointer>() {
        uefi::println!("  find_handles::<Pointer> => {} handles", handles.len());
        for h in handles {
            if let Ok(mut p) = open_protocol_exclusive::<Pointer>(h) {
                let _ = p.reset(false);
                v.push(p);
            }
        }
    }
    if v.is_empty() {
        if let Ok(h) = boot::get_handle_for_protocol::<Pointer>() {
            if let Ok(mut p) = open_protocol_exclusive::<Pointer>(h) {
                let _ = p.reset(false);
                v.push(p);
            }
        }
    }
    v
}

fn probe_fat_root_hint() -> String {
    let handles = list_simple_fs_handles();
    if handles.is_empty() {
        return String::from("Volume: no FAT (Simple FS)");
    }
    for h in handles {
        if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
            let mut io = SimpleFsIo { fs: &mut *fs };
            match io.list(&[]) {
                Ok(entries) => {
                    return format!("Volume: FAT root, {} entries", entries.len());
                }
                Err(_) => {}
            }
        }
    }
    String::from("Volume: FAT present (unreadable)")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CursorShape {
    Arrow,
    IBeam,
    Hand,
    /// Horizontal resize ↔ (left/right window edge)
    ResizeH,
    /// Vertical resize ↕ (top/bottom window edge)
    ResizeV,
    /// Diagonal resize ↖↘ (top-left / bottom-right corner)
    ResizeNWSE,
    /// Diagonal resize ↗↙ (top-right / bottom-left corner)
    ResizeNESW,
}

fn map_uefi_to_key_event(k: UefiKey) -> Option<KeyEvent> {
    match k {
        UefiKey::Special(ScanCode::LEFT) => Some(KeyEvent::new(Key::Left)),
        UefiKey::Special(ScanCode::RIGHT) => Some(KeyEvent::new(Key::Right)),
        UefiKey::Special(ScanCode::UP) => Some(KeyEvent::new(Key::Up)),
        UefiKey::Special(ScanCode::DOWN) => Some(KeyEvent::new(Key::Down)),
        UefiKey::Special(ScanCode::HOME) => Some(KeyEvent::new(Key::Home)),
        UefiKey::Special(ScanCode::END) => Some(KeyEvent::new(Key::End)),
        UefiKey::Special(ScanCode::DELETE) => Some(KeyEvent::new(Key::Delete)),
        UefiKey::Special(ScanCode::PAGE_UP) => Some(KeyEvent::new(Key::PageUp)),
        UefiKey::Special(ScanCode::PAGE_DOWN) => Some(KeyEvent::new(Key::PageDown)),
        UefiKey::Special(ScanCode::ESCAPE) => Some(KeyEvent::new(Key::Escape)),
        UefiKey::Special(ScanCode::INSERT) => Some(KeyEvent::new(Key::Other(0x7F))),
        UefiKey::Special(ScanCode::FUNCTION_1) => Some(KeyEvent::new(Key::Function(1))),
        UefiKey::Special(ScanCode::FUNCTION_2) => Some(KeyEvent::new(Key::Function(2))),
        UefiKey::Special(ScanCode::FUNCTION_3) => Some(KeyEvent::new(Key::Function(3))),
        UefiKey::Special(ScanCode::FUNCTION_4) => Some(KeyEvent::new(Key::Function(4))),
        UefiKey::Special(ScanCode::FUNCTION_5) => Some(KeyEvent::new(Key::Function(5))),
        UefiKey::Special(ScanCode::FUNCTION_6) => Some(KeyEvent::new(Key::Function(6))),
        UefiKey::Special(ScanCode::FUNCTION_7) => Some(KeyEvent::new(Key::Function(7))),
        UefiKey::Special(ScanCode::FUNCTION_8) => Some(KeyEvent::new(Key::Function(8))),
        UefiKey::Special(ScanCode::FUNCTION_9) => Some(KeyEvent::new(Key::Function(9))),
        UefiKey::Special(ScanCode::FUNCTION_10) => Some(KeyEvent::new(Key::Function(10))),
        UefiKey::Special(ScanCode::FUNCTION_11) => Some(KeyEvent::new(Key::Function(11))),
        UefiKey::Special(ScanCode::FUNCTION_12) => Some(KeyEvent::new(Key::Function(12))),
        UefiKey::Printable(ch) => {
            let c: char = ch.into();
            if c == '\r' || c == '\n' {
                return Some(KeyEvent::new(Key::Enter));
            }
            if c == '\t' {
                return Some(KeyEvent::new(Key::Tab));
            }
            if c == '\u{8}' {
                return Some(KeyEvent::new(Key::Backspace));
            }
            let u = c as u32;
            if (1..=26).contains(&u) {
                let lc = char::from_u32(0x60 + u)?;
                return Some(KeyEvent::with_modifiers(
                    Key::Character(lc),
                    Modifiers {
                        ctrl: true,
                        shift: false,
                        alt: false,
                    },
                ));
            }
            Some(KeyEvent::new(Key::Character(c)))
        }
        _ => None,
    }
}

fn scroll_focusable(textarea: &TextArea, visible_lines: usize) -> bool {
    let vl = visible_lines.max(1);
    textarea.line_count().max(1) > vl
}

fn apply_scrollbar_keys(
    textarea: &mut TextArea,
    scroll: &mut ScrollbarState,
    ev: &KeyEvent,
    visible_lines: usize,
) {
    let vl = visible_lines.max(1);
    let max_top = textarea.line_count().saturating_sub(vl);
    match ev.key {
        Key::Up => {
            textarea.scroll_top_line = textarea.scroll_top_line.saturating_sub(1);
        }
        Key::Down => {
            textarea.scroll_top_line = (textarea.scroll_top_line + 1).min(max_top);
        }
        Key::PageUp => {
            textarea.scroll_top_line = textarea.scroll_top_line.saturating_sub(vl);
        }
        Key::PageDown => {
            textarea.scroll_top_line = (textarea.scroll_top_line + vl).min(max_top);
        }
        Key::Home => {
            textarea.scroll_top_line = 0;
        }
        Key::End => {
            textarea.scroll_top_line = max_top;
        }
        _ => {}
    }
    textarea.scroll_top_line = textarea.scroll_top_line.min(max_top);
    textarea_sync_vertical_scroll(textarea, visible_lines, 0, scroll);
}

fn merge_virtual_nav(ev: KeyEvent, virtual_shift: bool, virtual_alt: bool) -> KeyEvent {
    let nav = matches!(
        ev.key,
        Key::Left
            | Key::Right
            | Key::Up
            | Key::Down
            | Key::Home
            | Key::End
            | Key::PageUp
            | Key::PageDown
    );
    if !nav {
        return ev;
    }
    KeyEvent {
        key: ev.key,
        modifiers: Modifiers {
            shift: ev.modifiers.shift || virtual_shift,
            alt: ev.modifiers.alt || virtual_alt,
            ctrl: ev.modifiers.ctrl,
        },
    }
}

const CLIPBOARD_CAP: usize = 16384;

struct StaticClipboard {
    len: usize,
    data: [u8; CLIPBOARD_CAP],
}

static mut CLIPBOARD: StaticClipboard = StaticClipboard {
    len: 0,
    data: [0; CLIPBOARD_CAP],
};

fn clipboard_set(s: &str) {
    let b = s.as_bytes();
    let n = b.len().min(CLIPBOARD_CAP);
    unsafe {
        CLIPBOARD.data[..n].copy_from_slice(&b[..n]);
        CLIPBOARD.len = n;
    }
}

fn clipboard_get() -> Option<&'static str> {
    unsafe {
        if CLIPBOARD.len == 0 {
            return None;
        }
        core::str::from_utf8(&CLIPBOARD.data[..CLIPBOARD.len]).ok()
    }
}

fn byte_at_click(
    ta: &TextArea,
    scroll_top_line: usize,
    inner_top_left: Point,
    line_h: i32,
    char_w: u32,
    px: i32,
    py: i32,
) -> Option<usize> {
    let rel_y = py - inner_top_left.y;
    if rel_y < 0 {
        return None;
    }
    let lh = line_h.max(1) as usize;
    let line_idx = scroll_top_line + (rel_y as usize / lh);
    let rel_x = px - inner_top_left.x - 4;
    let col = if rel_x <= 0 {
        0
    } else {
        (rel_x as u32 / char_w) as usize
    };
    let lines = ta.lines();
    let line = lines.get(line_idx).copied()?;
    let b_in_line: usize = line.chars().take(col).map(|c| c.len_utf8()).sum();
    let mut pos = 0usize;
    for (i, l) in lines.iter().enumerate() {
        if i == line_idx {
            return Some(pos + b_in_line.min(l.len()));
        }
        pos += l.len();
        if pos < ta.text.len() {
            let b = ta.text.as_bytes()[pos];
            if b == b'\n' {
                pos += 1;
            }
        }
    }
    None
}

fn draw_cursor_arrow(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let black = Rgb888::new(0, 0, 0);
    let white = Rgb888::new(255, 255, 255);
    let shadow = Rgb888::new(128, 128, 128);

    let mut r = |dx: i32, dy: i32, w: u32, h: u32, c: Rgb888| {
        if w == 0 || h == 0 {
            return;
        }
        let px = x.saturating_add(dx);
        let py = y.saturating_add(dy);
        if px < 0 || py < 0 {
            return;
        }
        target.fill_rect_solid(px as u32, py as u32, w, h, c);
    };

    r(2, 2, 11, 15, shadow);
    r(0, 0, 2, 14, black);
    r(2, 0, 9, 2, black);
    r(2, 2, 7, 2, white);
    r(2, 4, 5, 2, white);
    r(2, 6, 3, 2, white);
    r(2, 8, 2, 4, white);
    r(4, 10, 2, 2, white);
    r(2, 12, 2, 2, black);
}

fn draw_cursor_ibeam(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let black = Rgb888::new(0, 0, 0);
    let white = Rgb888::new(255, 255, 255);
    let mut r = |dx: i32, dy: i32, w: u32, h: u32, c: Rgb888| {
        if w == 0 || h == 0 {
            return;
        }
        let px = x.saturating_add(dx);
        let py = y.saturating_add(dy);
        if px < 0 || py < 0 {
            return;
        }
        target.fill_rect_solid(px as u32, py as u32, w, h, c);
    };
    r(0, 0, 7, 2, black);
    r(0, 1, 7, 1, white);
    r(2, 2, 3, 12, black);
    r(3, 3, 1, 10, white);
    r(0, 14, 7, 2, black);
    r(0, 14, 7, 1, white);
}

fn draw_cursor_hand(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let black = Rgb888::new(0, 0, 0);
    let white = Rgb888::new(255, 255, 255);
    let mut r = |dx: i32, dy: i32, w: u32, h: u32, c: Rgb888| {
        if w == 0 || h == 0 {
            return;
        }
        let px = x.saturating_add(dx);
        let py = y.saturating_add(dy);
        if px < 0 || py < 0 {
            return;
        }
        target.fill_rect_solid(px as u32, py as u32, w, h, c);
    };
    // Index finger
    r(4, 0, 3, 2, black);
    r(5, 2, 1, 4, white);
    r(4, 2, 1, 5, black);
    r(6, 2, 1, 5, black);
    // Three additional fingers
    r(7, 5, 1, 2, black);
    r(8, 5, 1, 3, white);
    r(9, 5, 1, 2, black);
    r(10, 6, 1, 2, black);
    r(11, 6, 1, 3, white);
    r(12, 6, 1, 2, black);
    // Palm body
    r(3, 7, 1, 6, black);
    r(4, 7, 9, 1, black);
    r(4, 8, 9, 5, white);
    r(13, 8, 1, 5, black);
    // Wrist
    r(3, 13, 1, 2, black);
    r(4, 13, 8, 2, white);
    r(12, 13, 1, 2, black);
    r(4, 15, 8, 1, black);
}

/// ↔ Horizontal resize cursor — hotspot at center (7, 3). 15×7 px.
fn draw_cursor_resize_h(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let b = Rgb888::new(0, 0, 0);
    let w = Rgb888::new(255, 255, 255);
    let s = Rgb888::new(128, 128, 128);
    let x = x - 7; let y = y - 3;
    let mut r = |dx: i32, dy: i32, rw: u32, rh: u32, c: Rgb888| {
        if rw == 0 || rh == 0 { return; }
        let px = x.saturating_add(dx); let py = y.saturating_add(dy);
        if px < 0 || py < 0 { return; }
        target.fill_rect_solid(px as u32, py as u32, rw, rh, c);
    };
    r(1, 1, 15, 7, s);         // shadow
    r(2, 0, 11, 7, w);         // white halo — vertical band
    r(0, 2, 15, 3, w);         // white halo — horizontal band
    // Left arrowhead (tip at col 0, base at col 3)
    r(0, 3, 4, 1, b);
    r(1, 2, 3, 1, b); r(1, 4, 3, 1, b);
    r(2, 1, 2, 1, b); r(2, 5, 2, 1, b);
    r(3, 0, 1, 1, b); r(3, 6, 1, 1, b);
    // Bar top/bottom edges
    r(4, 2, 7, 1, b); r(4, 4, 7, 1, b);
    // Right arrowhead (base at col 11, tip at col 14)
    r(11, 3, 4, 1, b);
    r(11, 2, 3, 1, b); r(11, 4, 3, 1, b);
    r(11, 1, 2, 1, b); r(11, 5, 2, 1, b);
    r(11, 0, 1, 1, b); r(11, 6, 1, 1, b);
}

/// ↕ Vertical resize cursor — hotspot at center (3, 7). 7×15 px.
fn draw_cursor_resize_v(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let b = Rgb888::new(0, 0, 0);
    let w = Rgb888::new(255, 255, 255);
    let s = Rgb888::new(128, 128, 128);
    let x = x - 3; let y = y - 7;
    let mut r = |dx: i32, dy: i32, rw: u32, rh: u32, c: Rgb888| {
        if rw == 0 || rh == 0 { return; }
        let px = x.saturating_add(dx); let py = y.saturating_add(dy);
        if px < 0 || py < 0 { return; }
        target.fill_rect_solid(px as u32, py as u32, rw, rh, c);
    };
    r(1, 1, 7, 15, s);
    r(0, 2, 7, 11, w);
    r(2, 0, 3, 15, w);
    // Top arrowhead
    r(3, 0, 1, 4, b);
    r(2, 1, 3, 1, b); r(1, 2, 5, 1, b);
    r(0, 3, 7, 1, b);
    // Bar left/right edges
    r(2, 4, 1, 7, b); r(4, 4, 1, 7, b);
    // Bottom arrowhead
    r(3, 11, 1, 4, b);
    r(2, 13, 3, 1, b); r(1, 12, 5, 1, b);
    r(0, 11, 7, 1, b);
}

/// ↖↘ NW–SE diagonal resize cursor — hotspot at center (7, 7). 15×15 px.
fn draw_cursor_resize_nwse(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let b = Rgb888::new(0, 0, 0);
    let w = Rgb888::new(255, 255, 255);
    let s = Rgb888::new(128, 128, 128);
    let x = x - 7; let y = y - 7;
    let mut r = |dx: i32, dy: i32, rw: u32, rh: u32, c: Rgb888| {
        if rw == 0 || rh == 0 { return; }
        let px = x.saturating_add(dx); let py = y.saturating_add(dy);
        if px < 0 || py < 0 { return; }
        target.fill_rect_solid(px as u32, py as u32, rw, rh, c);
    };
    r(1, 1, 15, 15, s);
    // NW arrowhead (top-left corner)
    r(0, 0, 6, 1, w); r(0, 1, 5, 1, w); r(0, 2, 4, 1, w); r(0, 3, 3, 1, w);
    r(0, 4, 2, 1, w); r(1, 0, 1, 5, w);
    r(0, 0, 5, 1, b); r(0, 1, 4, 1, b); r(0, 2, 3, 1, b);
    r(0, 3, 2, 1, b); r(0, 4, 1, 1, b);
    r(1, 0, 1, 4, b); r(2, 0, 1, 3, b); r(3, 0, 1, 2, b); r(4, 0, 1, 1, b);
    // Diagonal bar
    for i in 0i32..5 {
        r(4 + i, 4 + i, 2, 2, w);
        r(5 + i, 5 + i, 1, 1, b);
    }
    // SE arrowhead (bottom-right corner)
    r(9, 14, 6, 1, w); r(10, 13, 5, 1, w); r(11, 12, 4, 1, w);
    r(12, 11, 3, 1, w); r(13, 10, 2, 1, w); r(14, 9, 1, 6, w);
    r(10, 14, 5, 1, b); r(11, 13, 4, 1, b); r(12, 12, 3, 1, b);
    r(13, 11, 2, 1, b); r(14, 10, 1, 1, b);
    r(14, 10, 1, 4, b); r(14, 11, 1, 3, b); r(14, 12, 1, 2, b); r(14, 13, 1, 1, b);
}

/// ↗↙ NE–SW diagonal resize cursor — hotspot at center (7, 7). 15×15 px.
fn draw_cursor_resize_nesw(target: &mut BgrxFramebuffer<'_>, x: i32, y: i32) {
    let b = Rgb888::new(0, 0, 0);
    let w = Rgb888::new(255, 255, 255);
    let s = Rgb888::new(128, 128, 128);
    let x = x - 7; let y = y - 7;
    let mut r = |dx: i32, dy: i32, rw: u32, rh: u32, c: Rgb888| {
        if rw == 0 || rh == 0 { return; }
        let px = x.saturating_add(dx); let py = y.saturating_add(dy);
        if px < 0 || py < 0 { return; }
        target.fill_rect_solid(px as u32, py as u32, rw, rh, c);
    };
    r(1, 1, 15, 15, s);
    // NE arrowhead (top-right corner)
    r(9, 0, 6, 1, w); r(10, 1, 5, 1, w); r(11, 2, 4, 1, w);
    r(12, 3, 3, 1, w); r(13, 4, 2, 1, w); r(14, 0, 1, 5, w);
    r(10, 0, 5, 1, b); r(11, 1, 4, 1, b); r(12, 2, 3, 1, b);
    r(13, 3, 2, 1, b); r(14, 4, 1, 1, b);
    r(14, 0, 1, 5, b); r(14, 1, 1, 4, b); r(14, 2, 1, 3, b); r(14, 3, 1, 2, b);
    // Diagonal bar (NE to SW)
    for i in 0i32..5 {
        r(10 - i, 4 + i, 2, 2, w);
        r(10 - i, 5 + i, 1, 1, b);
    }
    // SW arrowhead (bottom-left corner)
    r(0, 9, 6, 1, w); r(0, 10, 5, 1, w); r(0, 11, 4, 1, w);
    r(0, 12, 3, 1, w); r(0, 13, 2, 1, w); r(0, 9, 1, 6, w);
    r(0, 10, 5, 1, b); r(0, 11, 4, 1, b); r(0, 12, 3, 1, b);
    r(0, 13, 2, 1, b); r(0, 14, 1, 1, b);
    r(0, 10, 1, 4, b); r(0, 11, 1, 3, b); r(0, 12, 1, 2, b); r(0, 13, 1, 1, b);
}

/// Pixels from window edge that trigger a resize cursor.
const RESIZE_HIT: i32 = 6;

fn cursor_shape_at(px: i32, py: i32, layout: &UiLayout) -> CursorShape {
    let pt = Point::new(px, py);

    // Window edge resize zones — check before interior hit tests.
    let win = layout.win;
    let wx0 = win.top_left.x;
    let wy0 = win.top_left.y;
    let wx1 = wx0 + win.size.width as i32;
    let wy1 = wy0 + win.size.height as i32;
    // Only trigger when pointer is within the window bounding box (±RESIZE_HIT).
    let near_win = px >= wx0 - RESIZE_HIT && px <= wx1 + RESIZE_HIT
        && py >= wy0 - RESIZE_HIT && py <= wy1 + RESIZE_HIT;
    if near_win {
        let on_left   = px >= wx0 - RESIZE_HIT && px < wx0 + RESIZE_HIT;
        let on_right  = px > wx1 - RESIZE_HIT  && px <= wx1 + RESIZE_HIT;
        // Skip top-edge resize: title bar is used for dragging.
        let on_bottom = py > wy1 - RESIZE_HIT  && py <= wy1 + RESIZE_HIT;
        // Corners
        if on_bottom && on_left  { return CursorShape::ResizeNESW; }
        if on_bottom && on_right { return CursorShape::ResizeNWSE; }
        // Edges
        if on_left || on_right   { return CursorShape::ResizeH; }
        if on_bottom             { return CursorShape::ResizeV; }
    }

    if layout.text_inner.contains(pt) {
        CursorShape::IBeam
    } else if layout.gallery_inner.contains(pt)
        || layout.menu_cells.iter().any(|c| c.contains(pt))
        || layout.sb_rect.contains(pt)
    {
        CursorShape::Hand
    } else {
        CursorShape::Arrow
    }
}

/// One frame of pointer protocol stats (for F12 overlay + serial).
struct PointerDiag {
    n_simple: usize,
    n_abs: usize,
    simple_some: u32,
    simple_err: u32,
    rel_dx: i32,
    rel_dy: i32,
    simple_res: [u64; 3],
    abs_ok: u32,
    abs_bad_mode: u32,
    abs_err: u32,
    abs0_x: u64,
    abs0_y: u64,
    abs0_btn: u32,
    abs0_min_x: u64,
    abs0_max_x: u64,
    abs0_min_y: u64,
    abs0_max_y: u64,
    out_x: i32,
    out_y: i32,
    btn: bool,
    btn_right: bool,
    rel_dz: i32,
}

fn draw_pointer_diag_overlay(
    target: &mut BgrxFramebuffer<'_>,
    w: u32,
    h: u32,
    font: &MonoFont<'_>,
    d: &PointerDiag,
) {
    let lh = font.character_size.height as u32 + 2;
    let lines: [String; 10] = [
        format!("Pointer F12=toggle | sp={} abs={}", d.n_simple, d.n_abs),
        format!("rel: dx={} dy={} dz={} ev={} err={}", d.rel_dx, d.rel_dy, d.rel_dz, d.simple_some, d.simple_err),
        format!("res: {} {} {}", d.simple_res[0], d.simple_res[1], d.simple_res[2]),
        format!("abs: ok={} badmode={} err={}", d.abs_ok, d.abs_bad_mode, d.abs_err),
        format!("abs0: {} {} btn={}", d.abs0_x, d.abs0_y, d.abs0_btn),
        format!(
            "range: {}..{}  {}..{}",
            d.abs0_min_x, d.abs0_max_x, d.abs0_min_y, d.abs0_max_y
        ),
        format!("out: {} {} L={} R={}", d.out_x, d.out_y, d.btn, d.btn_right),
        if d.n_simple == 0 {
            String::from("sp=0: no SimplePointer (VM needs USB tablet / PS/2)")
        } else {
            String::from("sp>=1: if ev stays 0 on move, check mouse capture / usb-tablet")
        },
        String::from("QEMU: make qemu uses scripts/qemu-laptop-input.sh"),
        String::from("Scroll: mouse wheel scrolls textarea when focused"),
    ];
    let box_h = lines.len() as u32 * lh + 12;
    let y0 = h.saturating_sub(box_h + 6);
    let x0 = 4u32;
    let box_w = w.saturating_sub(8);
    target.fill_rect_solid(x0, y0, box_w, box_h, Rgb888::new(0x18, 0x18, 0x22));
    let _ = Rectangle::new(Point::new(x0 as i32, y0 as i32), Size::new(box_w, box_h))
        .into_styled(PrimitiveStyle::with_stroke(Rgb888::new(0xff, 0xd7, 0x00), 1))
        .draw(target);
    let fg = Rgb888::new(0xc8, 0xff, 0xc8);
    let style = MonoTextStyle::new(font, fg);
    for (i, line) in lines.iter().enumerate() {
        let y = y0 + 6 + i as u32 * lh;
        let _ = Text::with_baseline(
            line.as_str(),
            Point::new(x0 as i32 + 6, y as i32),
            style,
            Baseline::Top,
        )
        .draw(target);
    }
}

/// Give OVMF's USB driver a chance to process transactions, then return so the main loop can
/// poll `read_state` / `get_state`.  Two strategies:
///
/// 1. **Preferred**: `wait_for_event` with a short timer — blocks at most ~16 ms, wakes early
///    on keyboard/pointer input.  This is the most power-efficient path.
/// 2. **Fallback**: `check_event` on each wait-event + `stall(16 ms)`.  Used when the timer
///    event could not be created, or as a safety net if `wait_for_event` blocks unexpectedly.
fn wait_for_uefi_input_or_timer(
    buf: &mut Vec<Event>,
    stdin_wait: Option<&Event>,
    simple_ev: &[Option<Event>],
    abs_ev: &[Option<Event>],
    poll_timer: Option<&Event>,
) {
    // Always check_event on every wait-event first.  This kicks OVMF's USB driver even if
    // we later fall back to stall instead of wait_for_event.
    if let Some(e) = stdin_wait {
        let _ = boot::check_event(e);
    }
    for opt in simple_ev.iter().chain(abs_ev.iter()) {
        if let Some(ev) = opt {
            let _ = boot::check_event(ev);
        }
    }

    // Use wait_for_event with only the timer — this lets OVMF's USB periodic polling run.
    // Pointer/keyboard wait-event clones are NOT included (they may be the cause of hangs).
    if let Some(t) = poll_timer {
        if boot::set_timer(t, TimerTrigger::Relative(160_000)).is_ok() {
            buf.clear();
            buf.push(unsafe { t.unsafe_clone() });
            let _ = boot::wait_for_event(buf);
            return;
        }
    }
    // If no timer, stall as last resort (USB data won't arrive but loop won't hang).
    boot::stall(Duration::from_millis(16));
}

#[entry]
fn main() -> Status {
    if let Err(e) = uefi::helpers::init() {
        return e.status();
    }

    let gop_handle = match boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(h) => h,
        Err(_) => return Status::UNSUPPORTED,
    };
    let mut gop = match boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
        Ok(g) => g,
        Err(e) => return e.status(),
    };

    let in_handle = match boot::get_handle_for_protocol::<Input>() {
        Ok(h) => h,
        Err(_) => return Status::UNSUPPORTED,
    };
    let mut stdin = match boot::open_protocol_exclusive::<Input>(in_handle) {
        Ok(i) => i,
        Err(e) => return e.status(),
    };
    let stdin_wait = stdin.wait_for_key_event().ok();

    // Force OVMF to bind USB drivers to all controllers — without this, the USB tablet
    // may not have protocol handlers installed and pointer handles stay at 1 (ConSplitter only).
    uefi::println!("[init] connecting all controllers (USB enumeration)...");
    connect_all_controllers();
    uefi::println!("[init] done connecting controllers");

    // Open every Simple / Absolute pointer instance.
    let abs_pointers = collect_absolute_pointers();
    let mut simple_pointers = collect_simple_pointers();
    let simple_pointer_wait_events: Vec<Option<Event>> = simple_pointers
        .iter()
        .map(|sp| sp.wait_for_input_event().ok())
        .collect();
    let abs_pointer_wait_events: Vec<Option<Event>> = abs_pointers
        .iter()
        .map(|ap| ap.wait_for_input_event().ok())
        .collect();
    let poll_timer = unsafe { boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None) }
        .ok();

    uefi::println!("[uefi_ui_demo] poll_timer={}", if poll_timer.is_some() { "OK" } else { "FAILED" });
    uefi::println!("[uefi_ui_demo] stdin_wait={}", if stdin_wait.is_some() { "OK" } else { "NONE" });
    uefi::println!("[uefi_ui_demo] sp_wait_events={:?}", simple_pointer_wait_events.iter().map(|e| e.is_some()).collect::<Vec<_>>());
    uefi::println!("[uefi_ui_demo] abs_wait_events={:?}", abs_pointer_wait_events.iter().map(|e| e.is_some()).collect::<Vec<_>>());

    uefi::println!(
        "[uefi_ui_demo] SimplePointer={} AbsolutePointer={} (F12=diag overlay)",
        simple_pointers.len(),
        abs_pointers.len()
    );
    for (i, sp) in simple_pointers.iter().enumerate() {
        let r = sp.mode().resolution;
        uefi::println!("  SP[{}] resolution=[{},{},{}]", i, r[0], r[1], r[2]);
    }
    for (i, ap) in abs_pointers.iter().enumerate() {
        let m = ap.mode();
        uefi::println!(
            "  ABS[{}] x={}..{} y={}..{} attrs={}",
            i, m.absolute_min_x, m.absolute_max_x,
            m.absolute_min_y, m.absolute_max_y,
            m.attributes.bits()
        );
        match ap.get_state() {
            Ok(st) => uefi::println!("  ABS[{}] initial state: x={} y={} btn={}", i, st.current_x, st.current_y, st.active_buttons),
            Err(e) => uefi::println!("  ABS[{}] initial get_state error: {:?}", i, e),
        }
    }

    uefi::println!("[init] gop mode info...");
    let info = gop.current_mode_info();
    if info.pixel_format() != PixelFormat::Bgr {
        uefi::println!("[init] pixel format {:?} != Bgr, aborting", info.pixel_format());
        return Status::UNSUPPORTED;
    }

    let (w, h) = info.resolution();
    let stride_bytes = info.stride() * 4;
    uefi::println!("[init] {}x{} stride={}", w, h, stride_bytes);

    let fb_len = {
        let fb = gop.frame_buffer();
        fb.size()
    };
    let mut back_buf = Vec::new();
    if back_buf.try_reserve_exact(fb_len).is_err() {
        return Status::OUT_OF_RESOURCES;
    }
    back_buf.resize(fb_len, 0);

    uefi::println!("[init] theme + font...");
    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;
    let ttf_font = load_font(EB_GARAMOND_TTF).ok();
    uefi::println!("[init] png decode...");
    let png_decoded = decode_demo_png(DEMO_PNG_BYTES);
    uefi::println!("[init] nav + gallery...");
    let mut nav = NavBar::new(MENU_LABELS, SUBMENUS);
    let mut gallery = GalleryState::new();
    gallery.fs_hint = probe_fat_root_hint();
    uefi::println!("[init] textarea...");
    let mut textarea = TextArea::from_str(
        "Tab: menu, editor, scrollbar, widget gallery. F6: aux window. Esc twice: menu; Esc then A/C: select all / copy.\n\
         F2/F3: virtual Shift/Alt. Ctrl+C/V: copy/paste. Help About: modal. F12: pointer diagnostics.\n",
    );
    let mut focus = Focus::Editor;
    let mut esc_chord_pending = false;
    let mut popovers = PopoverStack::default();
    let mut win_stack = WindowStack::new(2);
    let mut aux_offset = WindowOffset::ZERO;
    let mut virtual_shift = false;
    let mut virtual_alt = false;
    let mut ptr_x = (w / 2) as i32;
    let mut ptr_y = (h / 2) as i32;
    let mut ptr_left: bool;
    let mut ptr_right: bool;
    let mut prev_ptr_left = false;
    let mut prev_ptr_right = false;
    // Per Absolute Pointer handle: last raw (x, y, buttons); we only snap the cursor when raw changes.
    let mut prev_abs_keys: Vec<Option<(u64, u64, u32)>> = alloc::vec![None; abs_pointers.len()];
    let mut scroll = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);

    let font = &FONT_6X10;
    let line_h = font.character_size.height as i32 + 3;
    let char_w = font.character_size.width as i32;

    let mut context_menu: Option<ContextMenuState> = None;
    // File picker dialog: (state, is_open_mode=true / save_mode=false)
    let mut file_picker: Option<(FilePickerDialogState, bool)> = None;
    let mut fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);
    let mut fp_drag: Option<FpDrag> = None;
    let mut need_paint = true;
    let mut repaint_scene = true;
    let mut show_pointer_diag = false;
    let mut pointer_diag = PointerDiag {
        n_simple: 0,
        n_abs: 0,
        simple_some: 0,
        simple_err: 0,
        rel_dx: 0,
        rel_dy: 0,
        simple_res: [0; 3],
        abs_ok: 0,
        abs_bad_mode: 0,
        abs_err: 0,
        abs0_x: 0,
        abs0_y: 0,
        abs0_btn: 0,
        abs0_min_x: 0,
        abs0_max_x: 0,
        abs0_min_y: 0,
        abs0_max_y: 0,
        out_x: 0,
        out_y: 0,
        btn: false,
        btn_right: false,
        rel_dz: 0,
    };
    let mut diag_serial_phase: u32 = 0;
    let mut wait_event_buf = Vec::with_capacity(2 + simple_pointers.len() + abs_pointers.len());

    uefi::println!("[init] entering main loop");
    loop {
        let mut dirty = need_paint;
        need_paint = false;
        diag_serial_phase = diag_serial_phase.wrapping_add(1);
        if diag_serial_phase <= 3 || diag_serial_phase % 60 == 0 {
            uefi::println!("[loop] frame={} dirty={}", diag_serial_phase, dirty);
        }

        wait_for_uefi_input_or_timer(
            &mut wait_event_buf,
            stdin_wait.as_ref(),
            &simple_pointer_wait_events,
            &abs_pointer_wait_events,
            poll_timer.as_ref(),
        );
        if diag_serial_phase <= 3 {
            uefi::println!("[loop] after wait");
        }

        loop {
            match stdin.read_key() {
                Ok(None) => break,
                Ok(Some(uk)) => {
                    dirty = true;
                    repaint_scene = true;
                    let Some(ev) = map_uefi_to_key_event(uk) else {
                        continue;
                    };
                    if matches!(ev.key, Key::Function(2)) {
                        virtual_shift = !virtual_shift;
                        continue;
                    }
                    if matches!(ev.key, Key::Function(3)) {
                        virtual_alt = !virtual_alt;
                        continue;
                    }
                    // Context menu keyboard: Escape closes; Up/Down navigate; Enter activates.
                    if context_menu.is_some() {
                        match ev.key {
                            Key::Escape => { context_menu = None; dirty = true; }
                            Key::Up => {
                                if let Some(ref mut cm) = context_menu {
                                    if cm.hovered > 0 { cm.hovered -= 1; dirty = true; }
                                }
                            }
                            Key::Down => {
                                if let Some(ref mut cm) = context_menu {
                                    if cm.hovered + 1 < CONTEXT_ITEMS.len() { cm.hovered += 1; dirty = true; }
                                }
                            }
                            Key::Enter => {
                                if let Some(cm) = context_menu.take() {
                                    if let Some(&Some(action)) = CONTEXT_ACTIONS.get(cm.hovered) {
                                        match action {
                                            ContextAction::SelectAll => textarea.select_all(),
                                            ContextAction::Copy => {
                                                if let Some(sel) = textarea.selected_text() { clipboard_set(&sel); }
                                            }
                                            ContextAction::Cut => {
                                                if let Some(sel) = textarea.selected_text() {
                                                    clipboard_set(&sel); let _ = textarea.replace_selection_with_str("");
                                                }
                                            }
                                            ContextAction::Paste => {
                                                if let Some(cb) = clipboard_get() {
                                                    let _ = textarea.replace_selection_with_str(cb);
                                                }
                                            }
                                        }
                                    }
                                    dirty = true; repaint_scene = true;
                                }
                            }
                            _ => { context_menu = None; dirty = true; }
                        }
                        continue;
                    }

                    if popovers.is_modal_blocking() {
                        if ev.key == Key::Escape {
                            let _ = popovers.pop();
                        }
                        continue;
                    }

                    if matches!(ev.key, Key::Function(6)) {
                        win_stack.focus_next();
                        continue;
                    }

                    if matches!(ev.key, Key::Function(12)) {
                        show_pointer_diag = !show_pointer_diag;
                        dirty = true;
                        repaint_scene = true;
                        uefi::println!(
                            "[uefi_ui_demo] pointer diag overlay: {}",
                            if show_pointer_diag { "ON" } else { "OFF" }
                        );
                        continue;
                    }

                    // File picker modal: consume all keys before normal dispatch.
                    if file_picker.is_some() {
                        let dw = (w * 4 / 5).min(560) as u32;
                        let dh = (h * 4 / 5).min(380) as u32;
                        let dlg = Rectangle::new(
                            Point::new((w as i32 - dw as i32) / 2, (h as i32 - dh as i32) / 2),
                            Size::new(dw, dh),
                        );
                        let layout_fp = compute_file_picker_layout(dlg, line_h, FP_SB_W);
                        let (mut fp, is_open) = file_picker.take().unwrap();
                        let handles = list_simple_fs_handles();
                        let action = {
                            let mut found = FilePickerDialogAction::None;
                            for &h in &handles {
                                if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                    let mut io = SimpleFsIo { fs: &mut *fs };
                                    found = fp.handle_key(&ev, layout_fp.visible_rows, &mut io)
                                        .unwrap_or(FilePickerDialogAction::Cancel);
                                    break;
                                }
                            }
                            found
                        };
                        match action {
                            FilePickerDialogAction::Confirm { path, name } => {
                                if is_open {
                                    for &h in &handles {
                                        if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                            let mut io = SimpleFsIo { fs: &mut *fs };
                                            if let Ok(bytes) = io.read_file(&path, &name) {
                                                let s = core::str::from_utf8(&bytes).unwrap_or("");
                                                textarea = TextArea::from_str(s);
                                            }
                                            break;
                                        }
                                    }
                                } else {
                                    for &h in &handles {
                                        if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                            let mut io = SimpleFsIo { fs: &mut *fs };
                                            let _ = io.write_file(&path, &name, textarea.text.as_bytes());
                                            break;
                                        }
                                    }
                                }
                                repaint_scene = true;
                            }
                            FilePickerDialogAction::Cancel => {
                                repaint_scene = true;
                            }
                            FilePickerDialogAction::None => {
                                let total = fp.picker.entries.len().max(1);
                                fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, total, layout_fp.visible_rows.max(1), fp.picker.scroll_top);
                                file_picker = Some((fp, is_open));
                                repaint_scene = true;
                            }
                        }
                        continue;
                    }

                    if win_stack.focused == 1 {
                        match ev.key {
                            Key::Left => aux_offset.nudge(-6, 0, -120, -80, 120, 80),
                            Key::Right => aux_offset.nudge(6, 0, -120, -80, 120, 80),
                            Key::Up => aux_offset.nudge(0, -6, -120, -80, 120, 80),
                            Key::Down => aux_offset.nudge(0, 6, -120, -80, 120, 80),
                            _ => {}
                        }
                        continue;
                    }

                    if focus == Focus::Editor && win_stack.focused == 0 {
                        if esc_chord_pending {
                            match ev.key {
                                Key::Character('a') | Key::Character('A') => {
                                    textarea.select_all();
                                    esc_chord_pending = false;
                                    continue;
                                }
                                Key::Character('c') | Key::Character('C') => {
                                    if let Some(t) = textarea.selected_text() {
                                        clipboard_set(&t);
                                    }
                                    esc_chord_pending = false;
                                    continue;
                                }
                                Key::Escape => {
                                    esc_chord_pending = false;
                                    focus = Focus::Menu;
                                    continue;
                                }
                                _ => {
                                    esc_chord_pending = false;
                                }
                            }
                        } else if ev.key == Key::Escape {
                            esc_chord_pending = true;
                            virtual_shift = false;
                            virtual_alt = false;
                            continue;
                        }
                    } else if ev.key == Key::Escape {
                        virtual_shift = false;
                        virtual_alt = false;
                    }

                    let layout_tab =
                        compute_layout(w, h, line_h, char_w as u32, aux_offset);
                    let sb_ok = scroll_focusable(&textarea, layout_tab.visible_lines);
                    if ev.key == Key::Tab {
                        if esc_chord_pending {
                            esc_chord_pending = false;
                        }
                        apply_tab(
                            &mut focus,
                            &mut gallery.gallery_focus,
                            sb_ok,
                            ev.modifiers.shift,
                        );
                        continue;
                    }
                    let ev = if matches!(focus, Focus::Editor) {
                        merge_virtual_nav(ev, virtual_shift, virtual_alt)
                    } else {
                        ev
                    };
                    match focus {
                        Focus::Menu => {
                            if let Some((ti, sub)) = nav.apply_key_event(&ev) {
                                match sub {
                                    Some(si) => {
                                        if ti == 3 && si == 1 {
                                            popovers.push(PopoverSpec {
                                                id: ABOUT_MODAL_ID,
                                                kind: PopoverKind::Modal,
                                            });
                                        }
                                        // File > New (ti=0, si=0), Open (si=1), Save (si=2)
                                        if ti == 0 && si == 0 {
                                            textarea = TextArea::from_str("");
                                            repaint_scene = true;
                                        } else if ti == 0 && (si == 1 || si == 2) {
                                            let mode = if si == 1 { PickerMode::Load } else { PickerMode::Save };
                                            let is_open = si == 1;
                                            let handles = list_simple_fs_handles();
                                            for &h in &handles {
                                                if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                                    let mut io = SimpleFsIo { fs: &mut *fs };
                                                    if let Ok(state) = FilePickerDialogState::new(mode, 2, &mut io) {
                                                        let total = state.picker.entries.len().max(1);
                                                        fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, total, 1, 0);
                                                        file_picker = Some((state, is_open));
                                                    }
                                                    break;
                                                }
                                            }
                                            repaint_scene = true;
                                        }
                                        if let Some(Some(items)) = SUBMENUS.get(ti) {
                                            if let Some(lbl) = items.get(si) {
                                                gallery.menu_line = format!(
                                                    "{} > {}",
                                                    MENU_LABELS[ti].replace('&', ""),
                                                    lbl.replace('&', ""),
                                                );
                                            }
                                        }
                                        nav.close_submenu();
                                    }
                                    None => {
                                        gallery.menu_line = format!("Bar: {}", MENU_LABELS[ti].replace('&', ""));
                                    }
                                }
                            }
                        }
                        Focus::Gallery => {
                            let _ = apply_gallery_key_event(
                                &mut gallery,
                                &ev,
                                virtual_shift,
                            );
                        }
                        Focus::Editor => {
                            if ev.modifiers.ctrl {
                                match ev.key {
                                    Key::Character('a') | Key::Character('A') => {
                                        textarea.select_all();
                                        continue;
                                    }
                                    Key::Character('c') | Key::Character('C') => {
                                        if let Some(t) = textarea.selected_text() {
                                            clipboard_set(&t);
                                        }
                                        continue;
                                    }
                                    Key::Character('x') | Key::Character('X') => {
                                        if let Some(t) = textarea.selected_text() {
                                            clipboard_set(&t);
                                            let _ = textarea.replace_selection_with_str("");
                                        }
                                        continue;
                                    }
                                    Key::Character('v') | Key::Character('V') => {
                                        if let Some(t) = clipboard_get() {
                                            let _ = textarea.replace_selection_with_str(t);
                                        }
                                        continue;
                                    }
                                    Key::Character('n') | Key::Character('N') => {
                                        textarea = TextArea::from_str("");
                                        repaint_scene = true;
                                        continue;
                                    }
                                    Key::Character('o') | Key::Character('O') => {
                                        let handles = list_simple_fs_handles();
                                        for &h in &handles {
                                            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                                let mut io = SimpleFsIo { fs: &mut *fs };
                                                if let Ok(state) = FilePickerDialogState::new(PickerMode::Load, 2, &mut io) {
                                                    let total = state.picker.entries.len().max(1);
                                                    fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, total, 1, 0);
                                                    file_picker = Some((state, true));
                                                }
                                                break;
                                            }
                                        }
                                        repaint_scene = true;
                                        continue;
                                    }
                                    Key::Character('s') | Key::Character('S') => {
                                        let handles = list_simple_fs_handles();
                                        for &h in &handles {
                                            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                                let mut io = SimpleFsIo { fs: &mut *fs };
                                                if let Ok(state) = FilePickerDialogState::new(PickerMode::Save, 2, &mut io) {
                                                    let total = state.picker.entries.len().max(1);
                                                    fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, total, 1, 0);
                                                    file_picker = Some((state, false));
                                                }
                                                break;
                                            }
                                        }
                                        repaint_scene = true;
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                            let _ = textarea.apply_key_event(&ev);
                        }
                        Focus::Scrollbar => {
                            apply_scrollbar_keys(
                                &mut textarea,
                                &mut scroll,
                                &ev,
                                layout_tab.visible_lines,
                            );
                        }
                    }
                }
                Err(e) => return e.status(),
            }
        }

        // Simple Pointer: relative deltas (PS/2, USB mouse). Drain until NOT_READY.
        pointer_diag.n_simple = simple_pointers.len();
        pointer_diag.n_abs = abs_pointers.len();
        pointer_diag.simple_some = 0;
        pointer_diag.simple_err = 0;
        pointer_diag.abs_ok = 0;
        pointer_diag.abs_bad_mode = 0;
        pointer_diag.abs_err = 0;
        if let Some(sp) = simple_pointers.first() {
            pointer_diag.simple_res = sp.mode().resolution;
        } else {
            pointer_diag.simple_res = [0; 3];
        }

        let mut simple_dx: i32 = 0;
        let mut simple_dy: i32 = 0;
        let mut simple_dz: i32 = 0;
        let mut simple_left = false;
        let mut simple_right = false;
        for (sp, wait_ev) in simple_pointers
            .iter_mut()
            .zip(simple_pointer_wait_events.iter())
        {
            // Adaptive scale: high-resolution devices (OVMF: 65536) produce tiny deltas
            // and need a larger multiplier; low-resolution devices (1) are already pixel-like.
            let res_x = sp.mode().resolution[0].max(1);
            let rel_scale: i32 = if res_x > 1000 { 4 } else if res_x > 1 { 2 } else { 1 };

            if let Some(ev) = wait_ev {
                let _ = boot::check_event(ev);
            }
            loop {
                match sp.read_state() {
                    Ok(None) => break,
                    Ok(Some(s)) => {
                        pointer_diag.simple_some += 1;
                        dirty = true;
                        simple_dx = simple_dx.saturating_add(
                            s.relative_movement[0].saturating_mul(rel_scale),
                        );
                        simple_dy = simple_dy.saturating_add(
                            s.relative_movement[1].saturating_mul(rel_scale),
                        );
                        simple_dz = simple_dz.saturating_add(s.relative_movement[2]);
                        simple_left |= s.button[0];
                        simple_right |= s.button[1];
                    }
                    Err(_) => {
                        pointer_diag.simple_err += 1;
                        break;
                    }
                }
            }
        }

        // 1) Always integrate relative motion first (works when Absolute Pointer is absent or stale).
        let mut px = (ptr_x.saturating_add(simple_dx)).clamp(0, w as i32 - 1);
        let mut py = (ptr_y.saturating_add(simple_dy)).clamp(0, h as i32 - 1);
        let mut pl = simple_left;
        let mut pr = simple_right;
        if simple_dx != 0 || simple_dy != 0 {
            dirty = true;
        }

        // 2) For each Absolute Pointer (tablet / absolute mouse): only **override** when raw state
        // **changes**. Do **not** re-apply the same sample every frame — that blocked PS/2 in
        // VirtualBox when a bogus absolute device reported a fixed position.
        for (i, ap) in abs_pointers.iter().enumerate() {
            match ap.get_state() {
                Ok(st) => {
                    let mode = ap.mode();
                    if mode.absolute_max_x > mode.absolute_min_x
                        && mode.absolute_max_y > mode.absolute_min_y
                    {
                        pointer_diag.abs_ok += 1;
                        if i == 0 {
                            pointer_diag.abs0_x = st.current_x;
                            pointer_diag.abs0_y = st.current_y;
                            pointer_diag.abs0_btn = st.active_buttons;
                            pointer_diag.abs0_min_x = mode.absolute_min_x;
                            pointer_diag.abs0_max_x = mode.absolute_max_x;
                            pointer_diag.abs0_min_y = mode.absolute_min_y;
                            pointer_diag.abs0_max_y = mode.absolute_max_y;
                        }
                        let key = abs_state_key(&st);
                        let prev = prev_abs_keys.get(i).copied().flatten();
                        let moved = prev.map(|p| p != key).unwrap_or(true);
                        prev_abs_keys[i] = Some(key);
                        if moved {
                            let (nx, ny) = map_abs_to_pixels(&st, mode, w, h);
                            let aleft = primary_button(&st);
                            let aright = right_button(&st);
                            if nx != px || ny != py || aleft != pl {
                                dirty = true;
                            }
                            px = nx;
                            py = ny;
                            pl = aleft || simple_left;
                            pr = aright || simple_right;
                        }
                    } else {
                        pointer_diag.abs_bad_mode += 1;
                    }
                }
                Err(_) => {
                    pointer_diag.abs_err += 1;
                }
            }
        }

        ptr_x = px;
        ptr_y = py;
        ptr_left = pl;
        ptr_right = pr;
        pointer_diag.rel_dx = simple_dx;
        pointer_diag.rel_dy = simple_dy;
        pointer_diag.rel_dz = simple_dz;
        pointer_diag.out_x = ptr_x;
        pointer_diag.out_y = ptr_y;
        pointer_diag.btn = ptr_left;
        pointer_diag.btn_right = ptr_right;

        // Always dump pointer stats to serial (every 60 frames ≈ 1s)
        if diag_serial_phase % 60 == 0 {
            uefi::println!(
                "[ptr] sp={} ev={} err={} rel={},{},{} abs ok={} bad={} err={} abs0={},{} out={},{}",
                pointer_diag.n_simple,
                pointer_diag.simple_some,
                pointer_diag.simple_err,
                pointer_diag.rel_dx,
                pointer_diag.rel_dy,
                pointer_diag.rel_dz,
                pointer_diag.abs_ok,
                pointer_diag.abs_bad_mode,
                pointer_diag.abs_err,
                pointer_diag.abs0_x,
                pointer_diag.abs0_y,
                pointer_diag.out_x,
                pointer_diag.out_y
            );
        }
        if show_pointer_diag {
            dirty = true;
            repaint_scene = true;
        }

        let layout = compute_layout(w, h, line_h, char_w as u32, aux_offset);
        if focus == Focus::Scrollbar && !scroll_focusable(&textarea, layout.visible_lines) {
            focus = Focus::Editor;
        }
        textarea.scroll_to_cursor(layout.visible_lines);
        textarea_sync_vertical_scroll(&textarea, layout.visible_lines, 0, &mut scroll);

        // Scroll wheel (Z-axis): scroll textarea when editor or scrollbar is focused.
        if simple_dz != 0
            && matches!(focus, Focus::Editor | Focus::Scrollbar)
        {
            // Negative Z = scroll down in most firmware; invert to match convention.
            let scroll_lines = -(simple_dz.signum());
            let vl = layout.visible_lines.max(1);
            let max_top = textarea.line_count().saturating_sub(vl);
            textarea.scroll_top_line = (textarea.scroll_top_line as i32 + scroll_lines)
                .clamp(0, max_top as i32) as usize;
            textarea_sync_vertical_scroll(&textarea, layout.visible_lines, 0, &mut scroll);
            dirty = true;
            repaint_scene = true;
        }

        // Context menu hover update (every frame while menu is open)
        if let Some(ref mut cm) = context_menu {
            let j = ((ptr_y - cm.rect.top_left.y - MENU_POPUP_TOP_PAD) / line_h)
                .max(0) as usize;
            let new_hov = j.min(CONTEXT_ITEMS.len().saturating_sub(1));
            if new_hov != cm.hovered {
                cm.hovered = new_hov;
                dirty = true;
            }
        }

        let click = ptr_left && !prev_ptr_left;

        // File picker mouse: drag affordance for list items (T-17).
        if file_picker.is_some() {
            if click {
                let dw = (w * 4 / 5).min(560) as u32;
                let dh = (h * 4 / 5).min(380) as u32;
                let dlg_fp = Rectangle::new(
                    Point::new((w as i32 - dw as i32) / 2, (h as i32 - dh as i32) / 2),
                    Size::new(dw, dh),
                );
                let layout_fp = compute_file_picker_layout(dlg_fp, line_h, FP_SB_W);
                let pt = Point::new(ptr_x, ptr_y);
                if layout_fp.list_inner.contains(pt) {
                    if let Some((ref fp, _)) = file_picker {
                        let row = ((ptr_y - layout_fp.list_inner.top_left.y) / line_h)
                            .max(0) as usize;
                        let abs_row = fp.picker.scroll_top + row;
                        if let Some(entry) = fp.picker.entries.get(abs_row) {
                            fp_drag = Some(FpDrag {
                                label: entry.name.clone(),
                                x: ptr_x,
                                y: ptr_y,
                            });
                        }
                    }
                } else {
                    fp_drag = None;
                }
                dirty = true;
                repaint_scene = true;
            }
            if ptr_left {
                if let Some(ref mut drag) = fp_drag {
                    if drag.x != ptr_x || drag.y != ptr_y {
                        drag.x = ptr_x;
                        drag.y = ptr_y;
                        dirty = true;
                        repaint_scene = true;
                    }
                }
            } else if fp_drag.is_some() {
                fp_drag = None;
                dirty = true;
                repaint_scene = true;
            }
        } else {
            fp_drag = None;
        }

        // Context menu activation: left-click on item, or dismiss
        if click && file_picker.is_none() {
            if let Some(cm) = context_menu.take() {
                dirty = true;
                repaint_scene = true;
                let pt2 = Point::new(ptr_x, ptr_y);
                if cm.rect.contains(pt2) {
                    let j = ((ptr_y - cm.rect.top_left.y - MENU_POPUP_TOP_PAD) / line_h)
                        .max(0) as usize;
                    if let Some(&Some(action)) = CONTEXT_ACTIONS.get(j) {
                        match action {
                            ContextAction::SelectAll => textarea.select_all(),
                            ContextAction::Copy => {
                                if let Some(sel) = textarea.selected_text() {
                                    clipboard_set(&sel);
                                }
                            }
                            ContextAction::Cut => {
                                if let Some(sel) = textarea.selected_text() {
                                    clipboard_set(&sel);
                                    let _ = textarea.replace_selection_with_str("");
                                }
                            }
                            ContextAction::Paste => {
                                if let Some(cb) = clipboard_get() {
                                    let _ = textarea.replace_selection_with_str(cb);
                                }
                            }
                        }
                    }
                }
                // Fall through — don't process normal click after context menu dismiss
                prev_ptr_left = ptr_left;
                continue;
            }
        }

        if click && file_picker.is_none() {
            dirty = true;
            repaint_scene = true;
            let ps = PointerState::new(ptr_x, ptr_y, true);
            let pt = Point::new(ptr_x, ptr_y);
            if let Some(i) = index_at(layout.menu_cells.as_slice(), &ps) {
                nav.pointer_activate_top(i);
                focus = Focus::Menu;
            } else {
                let mut popup_click = false;
                if let Some((ti, _)) = nav.open {
                    if let Some(Some(items)) = SUBMENUS.get(ti) {
                        if let Some(cell) = layout.menu_cells.get(ti) {
                            let popup = submenu_popup_rect(cell, items, char_w, line_h);
                            if popup.contains(pt) {
                                popup_click = true;
                                let j = ((ptr_y - popup.top_left.y - 4) / line_h).max(0) as usize;
                                if j < items.len() {
                                    nav.open = Some((ti, j));
                                    gallery.menu_line = format!(
                                        "{} > {}",
                                        MENU_LABELS[ti].replace('&', ""),
                                        items[j].replace('&', ""),
                                    );
                                    if ti == 3 && j == 1 {
                                        popovers.push(PopoverSpec {
                                            id: ABOUT_MODAL_ID,
                                            kind: PopoverKind::Modal,
                                        });
                                    }
                                    if ti == 0 && j == 0 {
                                        textarea = TextArea::from_str("");
                                        repaint_scene = true;
                                    } else if ti == 0 && (j == 1 || j == 2) {
                                        let mode = if j == 1 { PickerMode::Load } else { PickerMode::Save };
                                        let is_open = j == 1;
                                        let handles = list_simple_fs_handles();
                                        for &h in &handles {
                                            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                                let mut io = SimpleFsIo { fs: &mut *fs };
                                                if let Ok(state) = FilePickerDialogState::new(mode, 2, &mut io) {
                                                    let total = state.picker.entries.len().max(1);
                                                    fp_scroll = ScrollbarState::new(ScrollAxis::Vertical, total, 1, 0);
                                                    file_picker = Some((state, is_open));
                                                }
                                                break;
                                            }
                                        }
                                        repaint_scene = true;
                                    }
                                }
                                focus = Focus::Menu;
                            }
                        }
                    }
                }
                if !popup_click && nav.open.is_some() {
                    nav.close_submenu();
                }
                if !popup_click {
                    if layout.gallery_inner.contains(pt) {
                        focus = Focus::Gallery;
                        let _ = gallery_pointer_down(
                            &mut gallery,
                            ptr_x,
                            ptr_y,
                            &layout.gallery_inner,
                            line_h,
                        );
                    } else if layout.text_inner.contains(pt) {
                        focus = Focus::Editor;
                        if let Some(b) = byte_at_click(
                            &textarea,
                            textarea.scroll_top_line,
                            layout.text_inner.top_left,
                            line_h,
                            char_w as u32,
                            ptr_x,
                            ptr_y,
                        ) {
                            textarea.set_cursor(b);
                        }
                    } else if layout.sb_rect.contains(pt) {
                        focus = Focus::Scrollbar;
                        let ratio = (ptr_y - layout.sb_rect.top_left.y) as f32
                            / layout.sb_rect.size.height.max(1) as f32;
                        scroll.set_offset_from_ratio(ratio);
                        textarea.scroll_top_line = scroll.offset;
                    }
                }
            }
        }
        prev_ptr_left = ptr_left;

        // Right-click: open context menu in text area, or close menus / dismiss modals.
        let right_click = ptr_right && !prev_ptr_right;
        if right_click && file_picker.is_none() {
            if context_menu.is_some() {
                context_menu = None;
                dirty = true;
            } else if layout.text_inner.contains(Point::new(ptr_x, ptr_y))
                && !popovers.is_modal_blocking()
            {
                // Compute popup size from CONTEXT_ITEMS
                let max_chars = CONTEXT_ITEMS
                    .iter()
                    .map(|s| s.chars().filter(|&c| c != '&').count())
                    .max()
                    .unwrap_or(0);
                let pop_w = (max_chars as i32 * char_w + 24).max(80) as u32;
                let pop_h = (CONTEXT_ITEMS.len() as i32 * line_h + 8).max(1) as u32;
                // Clamp so popup stays on screen
                let cx = (ptr_x).min(w as i32 - pop_w as i32).max(0);
                let cy = (ptr_y).min(h as i32 - pop_h as i32).max(0);
                context_menu = Some(ContextMenuState {
                    rect: Rectangle::new(
                        Point::new(cx, cy),
                        Size::new(pop_w, pop_h),
                    ),
                    hovered: 0,
                });
                dirty = true;
            } else if popovers.is_modal_blocking() {
                let _ = popovers.pop();
                dirty = true;
                repaint_scene = true;
            } else if nav.open.is_some() {
                nav.close_submenu();
                dirty = true;
                repaint_scene = true;
            }
        }
        prev_ptr_right = ptr_right;

        if !dirty {
            boot::stall(Duration::from_millis(16));
            continue;
        }

        if repaint_scene {
            let Some(mut back_tgt) =
                BgrxFramebuffer::new(&mut back_buf, w as u32, h as u32, stride_bytes)
            else {
                return Status::DEVICE_ERROR;
            };
            let png_arg = png_decoded
                .as_ref()
                .map(|(buf, pw, ph)| (buf.as_slice(), *pw, *ph));
            paint_scene_no_cursor(
                &mut back_tgt,
                w as u32,
                h as u32,
                &theme,
                &bevel,
                font,
                ttf_font.as_ref(),
                &nav,
                &gallery,
                &textarea,
                png_arg,
                focus,
                &scroll,
                line_h,
                char_w,
                &layout,
                &popovers,
                &win_stack,
            );
            if show_pointer_diag {
                draw_pointer_diag_overlay(
                    &mut back_tgt,
                    w as u32,
                    h as u32,
                    font,
                    &pointer_diag,
                );
            }
            repaint_scene = false;
        }

        {
            let mut fb_mem = gop.frame_buffer();
            let base = fb_mem.as_mut_ptr();
            let len = fb_mem.size();
            let slice = unsafe { core::slice::from_raw_parts_mut(base, len) };
            if len != back_buf.len() {
                return Status::DEVICE_ERROR;
            }
            slice.copy_from_slice(&back_buf);

            let Some(mut target) = BgrxFramebuffer::new(slice, w as u32, h as u32, stride_bytes)
            else {
                return Status::DEVICE_ERROR;
            };
            // Context menu overlay (drawn on top of everything, before cursor)
            if let Some(ref cm) = context_menu {
                let _ = draw_menu_popup(
                    &mut target,
                    &bevel,
                    cm.rect,
                    CONTEXT_ITEMS,
                    cm.hovered,
                    line_h,
                    font,
                    theme.colors.canvas,
                    theme.colors.text,
                    theme.colors.selection_bg,
                    theme.colors.caption_on_accent,
                );
            }

            // File picker overlay
            if let Some((ref fp, is_open)) = file_picker {
                let dw = (w * 4 / 5).min(560) as u32;
                let dh = (h * 4 / 5).min(380) as u32;
                let dlg = Rectangle::new(
                    Point::new((w as i32 - dw as i32) / 2, (h as i32 - dh as i32) / 2),
                    Size::new(dw, dh),
                );
                let layout_fp = compute_file_picker_layout(dlg, line_h, FP_SB_W);
                let _ = draw_file_picker(
                    &mut target,
                    &bevel,
                    &layout_fp,
                    &fp.picker,
                    &fp.filename.text,
                    &["All files", "Text files (*.txt)"],
                    fp.filetype_sel,
                    is_open,
                    fp.focus,
                    line_h,
                    char_w,
                    font,
                    &theme.colors,
                    &fp_scroll,
                );

                // Drag affordance: floating item label follows cursor (T-17)
                if let Some(ref drag) = fp_drag {
                    let label_w = (drag.label.chars().count() as i32 * char_w + 12)
                        .max(40)
                        .min(w as i32 - 4);
                    let label_h = line_h + 4;
                    let lx = (drag.x + 8).min(w as i32 - label_w - 2).max(0);
                    let ly = (drag.y - label_h - 4).max(0).min(h as i32 - label_h - 2);
                    let drag_rect = Rectangle::new(
                        Point::new(lx, ly),
                        Size::new(label_w as u32, label_h as u32),
                    );
                    let _ = bevel.draw_raised(&mut target, drag_rect);
                    let _ = Text::with_baseline(
                        &drag.label,
                        Point::new(lx + 4, ly + 2),
                        uefi_ui::embedded_graphics::mono_font::MonoTextStyle::new(font, theme.colors.text),
                        uefi_ui::embedded_graphics::text::Baseline::Top,
                    ).draw(&mut target);
                }
            }

            let shape = cursor_shape_at(ptr_x, ptr_y, &layout);
            match shape {
                CursorShape::Arrow    => draw_cursor_arrow(&mut target, ptr_x, ptr_y),
                CursorShape::IBeam    => draw_cursor_ibeam(&mut target, ptr_x, ptr_y),
                CursorShape::Hand     => draw_cursor_hand(&mut target, ptr_x, ptr_y),
                CursorShape::ResizeH  => draw_cursor_resize_h(&mut target, ptr_x, ptr_y),
                CursorShape::ResizeV  => draw_cursor_resize_v(&mut target, ptr_x, ptr_y),
                CursorShape::ResizeNWSE => draw_cursor_resize_nwse(&mut target, ptr_x, ptr_y),
                CursorShape::ResizeNESW => draw_cursor_resize_nesw(&mut target, ptr_x, ptr_y),
            }
        }

        boot::stall(Duration::from_millis(16));
    }
}
