//! **Lotus OS** -- bootable UEFI text editor.
//!
//! Boots directly to a full-screen editor. File picker is rooted to user-accessible
//! USB/removable volumes (EFI partitions are hidden). Settings (last file, last directory)
//! are persisted in UEFI non-volatile RAM between boots.
//!
//! Key model:
//!   Esc (in editor)  → enter menu mode
//!   Esc (in menu)    → exit back to editor
//!   In menu mode:    f=File menu  e=Edit menu  n=New  o=Open  s=Save  q=Quit
//!   In File menu:    n=New  l=Open Last  o=Open  s=Save  a=Save As  q=Quit
//!   In Edit menu:    c=Copy  v=Paste  a=Select All  d=Deselect  f=Find
//!   In Find bar:     type query  Enter=next match  Esc=close
#![no_main]
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use uefi::boot::{
    self, open_protocol_exclusive, set_watchdog_timer, EventType, TimerTrigger, Tpl,
};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::console::text::{Input, Key as UefiKey, ScanCode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;

use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::bedrock_controls::{
    compute_file_picker_layout, draw_file_picker, draw_menu_bar, draw_menu_popup,
    draw_menu_popup_ex, draw_window_title_bar, FP_SB_W,
};
use uefi_ui::editor_settings::EditorSettings;
use uefi_ui::embedded_graphics::mono_font::ascii::FONT_6X10;
use uefi_ui::embedded_graphics::mono_font::MonoTextStyle;
use uefi_ui::embedded_graphics::prelude::*;
use uefi_ui::embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use uefi_ui::embedded_graphics::text::{Baseline, Text};
use uefi_ui::embedded_graphics::geometry::{Point, Size};
use uefi_ui::file_picker::{FileIo, FilePickerDialogAction, FilePickerDialogState, LineInput, PickerMode};
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::input::{Key, KeyEvent, Modifiers};
use uefi_ui::theme::Theme;
use uefi_ui::uefi_vars::{load_settings_blob, save_settings_blob};
use uefi_ui::{find_user_fs_handles, list_simple_fs_handles, SimpleFsIo};
use uefi_ui::widgets::{ScrollAxis, ScrollbarState, TextArea};

// ── Layout constants ──────────────────────────────────────────────────────────
const TITLE_H: i32 = 20;
const MENU_H:  i32 = 20;
const SB_W:    i32 = 18;
const LINE_H:  i32 = 10;
const CHAR_W:  i32 = 6;
const FIND_H:  i32 = 20;

// ── Menu definitions ──────────────────────────────────────────────────────────
// Shown in the menu bar
const MENU_LABELS: &[&str] = &["&File", "&Edit"];

// File menu — mnemonic letters match single-key shortcuts when dropdown is open
const FILE_MENU: &[&str] = &[
    "&New",
    "Open &Last File",
    "&Open...",
    "&Save",
    "S&ave As...",
    "--",
    "&Quit",
];
const FILE_IDX_NEW:       usize = 0;
const FILE_IDX_OPEN_LAST: usize = 1;
const FILE_IDX_OPEN:      usize = 2;
const FILE_IDX_SAVE:      usize = 3;
const FILE_IDX_SAVE_AS:   usize = 4;
const FILE_IDX_QUIT:      usize = 6;

// Edit menu
const EDIT_MENU: &[&str] = &[
    "&Copy",
    "&Paste",
    "Select &All",
    "&Deselect",
    "--",
    "&Find",
];
const EDIT_IDX_COPY:       usize = 0;
const EDIT_IDX_PASTE:      usize = 1;
const EDIT_IDX_SELECT_ALL: usize = 2;
const EDIT_IDX_DESELECT:   usize = 3;
const EDIT_IDX_FIND:       usize = 5;

// ── App mode ──────────────────────────────────────────────────────────────────
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Editing,
    MenuBar,    // menu bar focused, no dropdown
    FileMenu,   // File dropdown open
    EditMenu,   // Edit dropdown open
    Find,
    FilePicker,
}

// ── Settings ──────────────────────────────────────────────────────────────────
fn settings_var_name() -> CString16 {
    CString16::try_from("LotusSettings").unwrap_or_else(|_| CString16::try_from("L").unwrap())
}

fn load_settings() -> EditorSettings {
    let name = settings_var_name();
    load_settings_blob(&name).ok().flatten()
        .and_then(|b| EditorSettings::from_blob(&b))
        .unwrap_or_default()
}

fn save_settings(s: &EditorSettings) {
    let _ = save_settings_blob(&settings_var_name(), &s.to_blob());
}

// ── Key mapping ──────────────────────────────────────────────────────────────
fn map_key(k: UefiKey) -> Option<KeyEvent> {
    match k {
        UefiKey::Special(ScanCode::LEFT)        => Some(KeyEvent::new(Key::Left)),
        UefiKey::Special(ScanCode::RIGHT)       => Some(KeyEvent::new(Key::Right)),
        UefiKey::Special(ScanCode::UP)          => Some(KeyEvent::new(Key::Up)),
        UefiKey::Special(ScanCode::DOWN)        => Some(KeyEvent::new(Key::Down)),
        UefiKey::Special(ScanCode::HOME)        => Some(KeyEvent::new(Key::Home)),
        UefiKey::Special(ScanCode::END)         => Some(KeyEvent::new(Key::End)),
        UefiKey::Special(ScanCode::DELETE)      => Some(KeyEvent::new(Key::Delete)),
        UefiKey::Special(ScanCode::PAGE_UP)     => Some(KeyEvent::new(Key::PageUp)),
        UefiKey::Special(ScanCode::PAGE_DOWN)   => Some(KeyEvent::new(Key::PageDown)),
        UefiKey::Special(ScanCode::ESCAPE)      => Some(KeyEvent::new(Key::Escape)),
        UefiKey::Printable(ch) => {
            let c: char = ch.into();
            if c == '\r' || c == '\n' { return Some(KeyEvent::new(Key::Enter)); }
            if c == '\t'              { return Some(KeyEvent::new(Key::Tab)); }
            if c == '\u{8}'           { return Some(KeyEvent::new(Key::Backspace)); }
            // Map Ctrl+letter (UEFI sends them as raw control chars 1-26)
            let u = c as u32;
            if (1..=26).contains(&u) {
                let lc = char::from_u32(0x60 + u)?;
                return Some(KeyEvent::with_modifiers(
                    Key::Character(lc),
                    Modifiers { ctrl: true, ..Default::default() },
                ));
            }
            Some(KeyEvent::new(Key::Character(c)))
        }
        _ => None,
    }
}

// ── Popup geometry ────────────────────────────────────────────────────────────
fn popup_rect_for(cell: &Rectangle, items: &[&str]) -> Rectangle {
    let max_ch = items.iter()
        .map(|s| s.chars().filter(|&c| c != '&' && c != '-').count())
        .max().unwrap_or(0) as i32;
    let pw = (max_ch * CHAR_W + 20).max(100) as u32;
    let ph = (items.len() as i32 * LINE_H + 8).max(4) as u32;
    Rectangle::new(
        Point::new(cell.top_left.x, cell.top_left.y + cell.size.height as i32),
        Size::new(pw, ph),
    )
}

// ── Open file picker ──────────────────────────────────────────────────────────
fn open_picker(mode: PickerMode, last_dir: &[String]) -> Option<FilePickerDialogState> {
    let user = find_user_fs_handles();
    let all  = list_simple_fs_handles();
    let h = user.first().or_else(|| all.first()).copied()?;
    let mut fs = open_protocol_exclusive::<SimpleFileSystem>(h).ok()?;
    let mut io = SimpleFsIo { fs: &mut *fs };
    FilePickerDialogState::with_root(mode, 2, alloc::vec![], last_dir.to_vec(), &mut io).ok()
}

// ── Auto-load last file on boot ───────────────────────────────────────────────
fn try_load_last_file(settings: &EditorSettings) -> Option<(Vec<u8>, Vec<String>, String)> {
    if settings.last_file.is_empty() { return None; }
    let last_dir = settings.last_dir_path();
    let user = find_user_fs_handles();
    let all  = list_simple_fs_handles();
    let h = user.first().or_else(|| all.first()).copied()?;
    let mut fs = open_protocol_exclusive::<SimpleFileSystem>(h).ok()?;
    let mut io = SimpleFsIo { fs: &mut *fs };
    let bytes = io.read_file(&last_dir, &settings.last_file).ok()?;
    Some((bytes, last_dir, settings.last_file.clone()))
}

// ── Find: all match byte ranges ───────────────────────────────────────────────
fn find_in(text: &str, query: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if query.is_empty() { return out; }
    let mut start = 0;
    while let Some(pos) = text[start..].find(query) {
        let lo = start + pos;
        let hi = lo + query.len();
        out.push((lo, hi));
        start = lo + 1;
        if start >= text.len() { break; }
    }
    out
}

// ── Status bar hint text per mode ─────────────────────────────────────────────
fn mode_hint(mode: Mode) -> &'static str {
    match mode {
        Mode::Editing  => "Esc: menu",
        Mode::MenuBar  => "f: File  e: Edit  n: New  o: Open  s: Save  q: Quit  Esc: back",
        Mode::FileMenu => "n: New  l: Open Last  o: Open  s: Save  a: Save As  q: Quit  Esc: back",
        Mode::EditMenu => "c: Copy  v: Paste  a: Select All  d: Deselect  f: Find  Esc: back",
        Mode::Find     => "type query  Enter: next match  Esc: close",
        Mode::FilePicker => "arrows: navigate  Enter: confirm  Esc: cancel",
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────
#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    // Connect all handles so USB HID drivers bind
    if let Ok(handles) = boot::locate_handle_buffer(uefi::boot::SearchType::AllHandles) {
        for &h in handles.iter() { let _ = boot::connect_controller(h, None, None, true); }
    }

    // Disable UEFI watchdog (default 5-min timeout reboots the machine)
    let _ = set_watchdog_timer(0, 0x10000, None);

    // ── GOP ─────────────────────────────────────────────────────────────────
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();
    let info = gop.current_mode_info();
    if info.pixel_format() != PixelFormat::Bgr {
        uefi::println!("[lotus-os] pixel format {:?} unsupported", info.pixel_format());
        return Status::UNSUPPORTED;
    }
    let (w, h) = info.resolution();
    let stride_bytes = info.stride() * 4;
    let fb_size = gop.frame_buffer().size();

    // ── Back buffer ──────────────────────────────────────────────────────────
    let mut back: Vec<u8> = Vec::new();
    if back.try_reserve_exact(fb_size).is_err() { return Status::OUT_OF_RESOURCES; }
    back.resize(fb_size, 0);

    // ── Settings + theme ─────────────────────────────────────────────────────
    let mut settings = load_settings();
    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;
    let font  = &FONT_6X10;

    // ── Layout ───────────────────────────────────────────────────────────────
    let status_h   = LINE_H + 4;
    let editor_top = TITLE_H + MENU_H;
    let editor_bot = h as i32 - status_h;
    let editor_h   = (editor_bot - editor_top).max(0) as u32;
    let vis_lines  = (editor_h as i32 / LINE_H).max(0) as usize;

    let title_rect    = Rectangle::new(Point::zero(), Size::new(w as u32, TITLE_H as u32));
    let menu_strip    = Rectangle::new(Point::new(0, TITLE_H), Size::new(w as u32, MENU_H as u32));
    let editor_rect   = Rectangle::new(Point::new(0, editor_top), Size::new(w as u32, editor_h));
    let sb_rect       = Rectangle::new(Point::new(w as i32 - SB_W, editor_top), Size::new(SB_W as u32, editor_h));
    let inner_rect    = Rectangle::new(
        Point::new(1, editor_top + 1),
        Size::new((w as i32 - SB_W - 2).max(0) as u32, editor_h.saturating_sub(2)),
    );
    let status_rect   = Rectangle::new(Point::new(0, editor_bot), Size::new(w as u32, status_h as u32));
    let find_bar_rect = Rectangle::new(Point::new(0, editor_bot - FIND_H), Size::new(w as u32, FIND_H as u32));

    // Menu cell geometry
    let menu_cells: Vec<Rectangle> = {
        let mut cells = Vec::new();
        let mut mx = 0i32;
        for lbl in MENU_LABELS {
            let cw = (lbl.chars().filter(|&c| c != '&').count() as i32 * CHAR_W + 8).max(24);
            cells.push(Rectangle::new(Point::new(mx, TITLE_H), Size::new(cw as u32, MENU_H as u32)));
            mx += cw;
        }
        cells
    };

    // ── App state ─────────────────────────────────────────────────────────────
    let mut textarea = TextArea::from_str("");
    let mut current_file: Option<(Vec<String>, String)> = None;
    let mut last_dir = settings.last_dir_path();
    let mut picker_mode = PickerMode::Load;

    // Auto-open last file on boot
    if let Some((bytes, dir, name)) = try_load_last_file(&settings) {
        let text = core::str::from_utf8(&bytes).unwrap_or("[binary]");
        textarea = TextArea::from_str(text);
        current_file = Some((dir.clone(), name.clone()));
        last_dir = dir;
    }

    let mut clipboard:   String = String::new();
    let mut mode:        Mode   = Mode::Editing;
    let mut menu_sel:    usize  = 0;  // selected item within open dropdown
    let mut menu_focus:  usize  = 0;  // focused tab in menu bar (File=0, Edit=1)

    // Find bar
    let mut find_input:   LineInput       = LineInput::new();
    let mut find_list:    Vec<(usize, usize)> = Vec::new();
    let mut find_current: usize           = 0;

    // File picker
    let mut file_picker: Option<FilePickerDialogState> = None;
    let mut fp_sb = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);

    let mut status: String = String::from(mode_hint(Mode::Editing));

    // ── Input ─────────────────────────────────────────────────────────────────
    let stdin = boot::get_handle_for_protocol::<Input>().unwrap();
    let mut kb = open_protocol_exclusive::<Input>(stdin).unwrap();

    let timer = unsafe {
        boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap()
    };
    boot::set_timer(&timer, TimerTrigger::Periodic(166_666)).unwrap();

    uefi::println!("[lotus-os] {}x{} ready", w, h);

    let mut dirty = true;

    loop {
        let mut events = [kb.wait_for_key_event().unwrap(), unsafe { timer.unsafe_clone() }];
        let _ = boot::wait_for_event(&mut events);

        while let Ok(Some(raw)) = kb.read_key() {
            let Some(ev) = map_key(raw) else { continue };

            // ── File picker ──────────────────────────────────────────────────
            if mode == Mode::FilePicker {
                let mut fp = file_picker.take().unwrap();
                let user = find_user_fs_handles();
                let all  = list_simple_fs_handles();
                let action = if let Some(fh) = user.first().or_else(|| all.first()).copied() {
                    if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(fh) {
                        let mut io = SimpleFsIo { fs: &mut *fs };
                        fp.handle_key(&ev, vis_lines, &mut io)
                            .unwrap_or(FilePickerDialogAction::None)
                    } else { FilePickerDialogAction::Cancel }
                } else { FilePickerDialogAction::Cancel };

                match action {
                    FilePickerDialogAction::Confirm { path, name } => {
                        last_dir = fp.last_dir.clone();
                        let user2 = find_user_fs_handles();
                        let all2  = list_simple_fs_handles();
                        if let Some(fh) = user2.first().or_else(|| all2.first()).copied() {
                            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(fh) {
                                let mut io = SimpleFsIo { fs: &mut *fs };
                                if picker_mode == PickerMode::Load {
                                    match io.read_file(&path, &name) {
                                        Ok(bytes) => {
                                            textarea = TextArea::from_str(
                                                core::str::from_utf8(&bytes).unwrap_or("[binary]"));
                                            current_file = Some((path, name.clone()));
                                            settings.last_file = name.clone();
                                            settings.set_last_dir_path(&last_dir);
                                            save_settings(&settings);
                                            status = format!("Opened: {}", name);
                                        }
                                        Err(_) => { status = format!("Error reading: {}", name); }
                                    }
                                } else {
                                    match io.write_file(&path, &name, textarea.text.as_bytes()) {
                                        Ok(()) => {
                                            current_file = Some((path, name.clone()));
                                            settings.last_file = name.clone();
                                            settings.set_last_dir_path(&last_dir);
                                            save_settings(&settings);
                                            status = format!("Saved: {}", name);
                                        }
                                        Err(_) => { status = format!("Error saving: {}", name); }
                                    }
                                }
                            }
                        }
                        mode = Mode::Editing;
                        status = format!("{} -- Esc: menu", status);
                    }
                    FilePickerDialogAction::Cancel => {
                        last_dir = fp.last_dir.clone();
                        mode = Mode::Editing;
                        status = String::from(mode_hint(Mode::Editing));
                    }
                    FilePickerDialogAction::None => {
                        let n = fp.picker.entries.len().max(1);
                        fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), fp.picker.scroll_top);
                        file_picker = Some(fp);
                    }
                }
                dirty = true;
                continue;
            }

            // ── Find bar ─────────────────────────────────────────────────────
            if mode == Mode::Find {
                match ev.key {
                    Key::Escape => {
                        mode = Mode::Editing;
                        find_list.clear();
                        status = String::from(mode_hint(Mode::Editing));
                    }
                    Key::Enter => {
                        if !find_list.is_empty() {
                            find_current = (find_current + 1) % find_list.len();
                            status = format!("Match {} of {}  Enter: next  Esc: close",
                                find_current + 1, find_list.len());
                        }
                    }
                    _ => {
                        find_input.apply_key(ev.key);
                        find_list = find_in(&textarea.text, &find_input.text);
                        find_current = 0;
                        status = if find_list.is_empty() {
                            format!("No matches for \"{}\"  Esc: close", find_input.text)
                        } else {
                            format!("{} match(es) for \"{}\"  Enter: next  Esc: close",
                                find_list.len(), find_input.text)
                        };
                    }
                }
                dirty = true;
                continue;
            }

            // ── Menu bar (no dropdown) ────────────────────────────────────────
            if mode == Mode::MenuBar {
                let ch = match ev.key { Key::Character(c) => Some(c.to_ascii_lowercase()), _ => None };
                match ev.key {
                    Key::Escape => {
                        mode = Mode::Editing;
                        status = String::from(mode_hint(Mode::Editing));
                    }
                    Key::Left => {
                        menu_focus = menu_focus.saturating_sub(1);
                        status = String::from(mode_hint(Mode::MenuBar));
                    }
                    Key::Right => {
                        menu_focus = (menu_focus + 1).min(MENU_LABELS.len() - 1);
                        status = String::from(mode_hint(Mode::MenuBar));
                    }
                    Key::Enter | Key::Down => {
                        // Open currently focused menu
                        if menu_focus == 0 {
                            mode = Mode::FileMenu;
                            menu_sel = FILE_IDX_NEW;
                        } else {
                            mode = Mode::EditMenu;
                            menu_sel = EDIT_IDX_COPY;
                        }
                        status = String::from(mode_hint(mode));
                    }
                    _ => {}
                }
                if let Some(c) = ch {
                    match c {
                        'f' => { mode = Mode::FileMenu; menu_sel = FILE_IDX_NEW; status = String::from(mode_hint(Mode::FileMenu)); }
                        'e' => { mode = Mode::EditMenu; menu_sel = EDIT_IDX_COPY; status = String::from(mode_hint(Mode::EditMenu)); }
                        'n' => { do_new(&mut textarea, &mut current_file); mode = Mode::Editing; status = String::from("New file.  Esc: menu"); }
                        'o' => {
                            picker_mode = PickerMode::Load;
                            match open_picker(PickerMode::Load, &last_dir) {
                                Some(fp) => {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), 0);
                                    file_picker = Some(fp);
                                    mode = Mode::FilePicker;
                                    status = String::from(mode_hint(Mode::FilePicker));
                                }
                                None => { mode = Mode::Editing; status = String::from("No accessible volume.  Esc: menu"); }
                            }
                        }
                        's' => {
                            do_save(&mut settings, &current_file, &textarea, &last_dir, &mut status);
                            if current_file.is_none() {
                                picker_mode = PickerMode::Save;
                                match open_picker(PickerMode::Save, &last_dir) {
                                    Some(fp) => {
                                        let n = fp.picker.entries.len().max(1);
                                        fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), 0);
                                        file_picker = Some(fp);
                                        mode = Mode::FilePicker;
                                        status = String::from(mode_hint(Mode::FilePicker));
                                    }
                                    None => { mode = Mode::Editing; status = String::from("No accessible volume.  Esc: menu"); }
                                }
                            } else {
                                mode = Mode::Editing;
                            }
                        }
                        'q' => {
                            settings.set_last_dir_path(&last_dir);
                            save_settings(&settings);
                            return Status::SUCCESS;
                        }
                        _ => {}
                    }
                }
                dirty = true;
                continue;
            }

            // ── File menu dropdown ────────────────────────────────────────────
            if mode == Mode::FileMenu {
                let ch = match ev.key { Key::Character(c) => Some(c.to_ascii_lowercase()), _ => None };
                let mut action: Option<usize> = None;

                match ev.key {
                    Key::Escape => { mode = Mode::MenuBar; menu_focus = 0; status = String::from(mode_hint(Mode::MenuBar)); }
                    Key::Up => {
                        // Move up, skip separator and disabled items
                        let mut next = menu_sel;
                        loop {
                            if next == 0 { break; }
                            next -= 1;
                            if FILE_MENU[next] == "--" { continue; }
                            if next == FILE_IDX_OPEN_LAST && settings.last_file.is_empty() { continue; }
                            break;
                        }
                        menu_sel = next;
                        status = String::from(mode_hint(Mode::FileMenu));
                    }
                    Key::Down => {
                        let mut next = menu_sel;
                        loop {
                            if next + 1 >= FILE_MENU.len() { break; }
                            next += 1;
                            if FILE_MENU[next] == "--" { continue; }
                            if next == FILE_IDX_OPEN_LAST && settings.last_file.is_empty() { continue; }
                            break;
                        }
                        menu_sel = next;
                        status = String::from(mode_hint(Mode::FileMenu));
                    }
                    Key::Enter => { action = Some(menu_sel); }
                    _ => {}
                }
                if let Some(c) = ch {
                    action = match c {
                        'n' => Some(FILE_IDX_NEW),
                        'l' if !settings.last_file.is_empty() => Some(FILE_IDX_OPEN_LAST),
                        'o' => Some(FILE_IDX_OPEN),
                        's' => Some(FILE_IDX_SAVE),
                        'a' => Some(FILE_IDX_SAVE_AS),
                        'q' => Some(FILE_IDX_QUIT),
                        _ => None,
                    };
                }

                if let Some(idx) = action {
                    match idx {
                        FILE_IDX_NEW => {
                            do_new(&mut textarea, &mut current_file);
                            mode = Mode::Editing;
                            status = String::from("New file.  Esc: menu");
                        }
                        FILE_IDX_OPEN_LAST => {
                            if let Some((bytes, dir, name)) = try_load_last_file(&settings) {
                                textarea = TextArea::from_str(core::str::from_utf8(&bytes).unwrap_or("[binary]"));
                                current_file = Some((dir.clone(), name.clone()));
                                last_dir = dir;
                                status = format!("Opened: {}  Esc: menu", name);
                            } else {
                                status = String::from("Could not open last file.  Esc: menu");
                            }
                            mode = Mode::Editing;
                        }
                        FILE_IDX_OPEN => {
                            picker_mode = PickerMode::Load;
                            match open_picker(PickerMode::Load, &last_dir) {
                                Some(fp) => {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), 0);
                                    file_picker = Some(fp);
                                    mode = Mode::FilePicker;
                                    status = String::from(mode_hint(Mode::FilePicker));
                                }
                                None => { mode = Mode::Editing; status = String::from("No accessible volume.  Esc: menu"); }
                            }
                        }
                        FILE_IDX_SAVE => {
                            if let Some((ref path, ref name)) = current_file.clone() {
                                let user = find_user_fs_handles();
                                let all  = list_simple_fs_handles();
                                if let Some(fh) = user.first().or_else(|| all.first()).copied() {
                                    if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(fh) {
                                        let mut io = SimpleFsIo { fs: &mut *fs };
                                        if io.write_file(path, name, textarea.text.as_bytes()).is_ok() {
                                            settings.last_file = name.clone();
                                            settings.set_last_dir_path(&last_dir);
                                            save_settings(&settings);
                                            status = format!("Saved: {}  Esc: menu", name);
                                        } else {
                                            status = format!("Error saving: {}  Esc: menu", name);
                                        }
                                    }
                                }
                                mode = Mode::Editing;
                            } else {
                                picker_mode = PickerMode::Save;
                                match open_picker(PickerMode::Save, &last_dir) {
                                    Some(fp) => {
                                        let n = fp.picker.entries.len().max(1);
                                        fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), 0);
                                        file_picker = Some(fp);
                                        mode = Mode::FilePicker;
                                        status = String::from(mode_hint(Mode::FilePicker));
                                    }
                                    None => { mode = Mode::Editing; status = String::from("No accessible volume.  Esc: menu"); }
                                }
                            }
                        }
                        FILE_IDX_SAVE_AS => {
                            picker_mode = PickerMode::Save;
                            match open_picker(PickerMode::Save, &last_dir) {
                                Some(fp) => {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, vis_lines.max(1), 0);
                                    file_picker = Some(fp);
                                    mode = Mode::FilePicker;
                                    status = String::from(mode_hint(Mode::FilePicker));
                                }
                                None => { mode = Mode::Editing; status = String::from("No accessible volume.  Esc: menu"); }
                            }
                        }
                        FILE_IDX_QUIT => {
                            settings.set_last_dir_path(&last_dir);
                            save_settings(&settings);
                            return Status::SUCCESS;
                        }
                        _ => { mode = Mode::MenuBar; status = String::from(mode_hint(Mode::MenuBar)); }
                    }
                }
                dirty = true;
                continue;
            }

            // ── Edit menu dropdown ────────────────────────────────────────────
            if mode == Mode::EditMenu {
                let ch = match ev.key { Key::Character(c) => Some(c.to_ascii_lowercase()), _ => None };
                let mut action: Option<usize> = None;

                match ev.key {
                    Key::Escape => { mode = Mode::MenuBar; menu_focus = 1; status = String::from(mode_hint(Mode::MenuBar)); }
                    Key::Up => {
                        let mut next = menu_sel;
                        loop {
                            if next == 0 { break; }
                            next -= 1;
                            if EDIT_MENU[next] != "--" { break; }
                        }
                        menu_sel = next;
                        status = String::from(mode_hint(Mode::EditMenu));
                    }
                    Key::Down => {
                        let mut next = menu_sel;
                        loop {
                            if next + 1 >= EDIT_MENU.len() { break; }
                            next += 1;
                            if EDIT_MENU[next] != "--" { break; }
                        }
                        menu_sel = next;
                        status = String::from(mode_hint(Mode::EditMenu));
                    }
                    Key::Enter => { action = Some(menu_sel); }
                    _ => {}
                }
                if let Some(c) = ch {
                    action = match c {
                        'c' => Some(EDIT_IDX_COPY),
                        'v' => Some(EDIT_IDX_PASTE),
                        'a' => Some(EDIT_IDX_SELECT_ALL),
                        'd' => Some(EDIT_IDX_DESELECT),
                        'f' => Some(EDIT_IDX_FIND),
                        _ => None,
                    };
                }

                if let Some(idx) = action {
                    match idx {
                        EDIT_IDX_COPY => {
                            if let Some(sel) = textarea.selected_text() {
                                clipboard = sel;
                                status = String::from("Copied.  Esc: menu");
                            } else {
                                status = String::from("Nothing selected.  Esc: menu");
                            }
                            mode = Mode::Editing;
                        }
                        EDIT_IDX_PASTE => {
                            for c in clipboard.clone().chars() {
                                textarea.apply_key_event(&KeyEvent::new(Key::Character(c)));
                            }
                            mode = Mode::Editing;
                            status = String::from("Pasted.  Esc: menu");
                        }
                        EDIT_IDX_SELECT_ALL => {
                            textarea.select_all();
                            mode = Mode::Editing;
                            status = String::from("Selected all.  Esc: menu");
                        }
                        EDIT_IDX_DESELECT => {
                            textarea.clear_selection();
                            mode = Mode::Editing;
                            status = String::from("Deselected.  Esc: menu");
                        }
                        EDIT_IDX_FIND => {
                            mode = Mode::Find;
                            find_input = LineInput::new();
                            find_list.clear();
                            find_current = 0;
                            status = String::from(mode_hint(Mode::Find));
                        }
                        _ => { mode = Mode::MenuBar; status = String::from(mode_hint(Mode::MenuBar)); }
                    }
                }
                dirty = true;
                continue;
            }

            // ── Editing mode ──────────────────────────────────────────────────
            // Esc enters menu mode; everything else goes to the textarea
            if ev.key == Key::Escape {
                mode = Mode::MenuBar;
                menu_focus = 0;
                status = String::from(mode_hint(Mode::MenuBar));
                dirty = true;
                continue;
            }

            textarea.apply_key_event(&ev);
            dirty = true;
        }

        if !dirty { continue; }
        dirty = false;

        // ── Paint ─────────────────────────────────────────────────────────────
        let Some(mut fb) = BgrxFramebuffer::new(&mut back, w as u32, h as u32, stride_bytes)
        else { return Status::DEVICE_ERROR; };

        fb.fill_rect_solid(0, 0, w as u32, h as u32, theme.colors.background);

        // Title bar
        let title_text = match &current_file {
            Some((_, n)) => format!("Lotus OS -- {}", n),
            None => String::from("Lotus OS -- [untitled]"),
        };
        let _ = draw_window_title_bar(
            &mut fb, title_rect, &title_text, font, true,
            theme.colors.accent, theme.colors.caption_on_accent, theme.colors.border,
        );

        // Which menu tab appears open for draw_menu_bar
        let menu_bar_open = match mode {
            Mode::MenuBar  => None,           // bar is focused but no dropdown yet
            Mode::FileMenu => Some(0usize),
            Mode::EditMenu => Some(1usize),
            _ => None,
        };
        // Highlight the focused tab in menu bar when in MenuBar mode
        let menu_bar_focused = if mode == Mode::MenuBar { Some(menu_focus) } else { menu_bar_open };

        let _ = draw_menu_bar(
            &mut fb, menu_strip, &menu_cells, MENU_LABELS,
            menu_bar_focused, font,
            theme.colors.surface, theme.colors.text, theme.colors.accent, theme.colors.caption_on_accent,
        );

        // Editor area
        let _ = bevel.draw_sunken(&mut fb, editor_rect);
        let edit_inner_h = if mode == Mode::Find {
            inner_rect.size.height.saturating_sub(FIND_H as u32 + 2)
        } else {
            inner_rect.size.height
        };
        let inner_vis = Rectangle::new(inner_rect.top_left, Size::new(inner_rect.size.width, edit_inner_h));
        Rectangle::new(inner_vis.top_left, inner_vis.size)
            .into_styled(PrimitiveStyle::with_fill(theme.colors.canvas))
            .draw(&mut fb)
            .ok();

        // Text lines
        let text_vis_lines = (edit_inner_h as i32 / LINE_H).max(0) as usize;
        for (li, line) in textarea.lines().iter()
            .enumerate()
            .skip(textarea.scroll_top_line)
            .take(text_vis_lines)
        {
            let vy = li - textarea.scroll_top_line;
            let y  = inner_vis.top_left.y + vy as i32 * LINE_H;
            let x  = inner_vis.top_left.x + 2;
            let _ = Text::with_baseline(
                line,
                Point::new(x, y + 1),
                MonoTextStyle::new(font, theme.colors.text),
                Baseline::Top,
            ).draw(&mut fb);
        }

        // Scrollbar
        let _ = bevel.draw_sunken(&mut fb, sb_rect);

        // Find bar
        if mode == Mode::Find {
            Rectangle::new(find_bar_rect.top_left, find_bar_rect.size)
                .into_styled(PrimitiveStyle::with_fill(theme.colors.surface))
                .draw(&mut fb)
                .ok();
            let label = format!("Find: {}_", find_input.text);
            let _ = Text::with_baseline(
                &label,
                Point::new(find_bar_rect.top_left.x + 4, find_bar_rect.top_left.y + 5),
                MonoTextStyle::new(font, theme.colors.text),
                Baseline::Top,
            ).draw(&mut fb);
        }

        // Status bar
        Rectangle::new(status_rect.top_left, status_rect.size)
            .into_styled(PrimitiveStyle::with_fill(theme.colors.surface))
            .draw(&mut fb)
            .ok();
        let _ = Text::with_baseline(
            &status,
            Point::new(status_rect.top_left.x + 4, status_rect.top_left.y + 2),
            MonoTextStyle::new(font, theme.colors.text),
            Baseline::Top,
        ).draw(&mut fb);

        // Menu popups
        if mode == Mode::FileMenu {
            if let Some(cell) = menu_cells.get(0) {
                let popup = popup_rect_for(cell, FILE_MENU);
                let no_last = settings.last_file.is_empty();
                let disabled = [false, no_last, false, false, false, false, false];
                let _ = draw_menu_popup_ex(
                    &mut fb, &bevel, popup, FILE_MENU, &disabled, menu_sel,
                    LINE_H, font,
                    theme.colors.canvas, theme.colors.text,
                    theme.colors.accent, theme.colors.caption_on_accent,
                );
            }
        }
        if mode == Mode::EditMenu {
            if let Some(cell) = menu_cells.get(1) {
                let popup = popup_rect_for(cell, EDIT_MENU);
                let _ = draw_menu_popup(
                    &mut fb, &bevel, popup, EDIT_MENU, menu_sel,
                    LINE_H, font,
                    theme.colors.canvas, theme.colors.text,
                    theme.colors.accent, theme.colors.caption_on_accent,
                );
            }
        }

        // File picker overlay
        if mode == Mode::FilePicker {
            if let Some(ref fp) = file_picker {
                let dw = (w as u32 * 4 / 5).min(560);
                let dh = (h as u32 * 4 / 5).min(380);
                let dlg = Rectangle::new(
                    Point::new((w as i32 - dw as i32) / 2, (h as i32 - dh as i32) / 2),
                    Size::new(dw, dh),
                );
                let layout_fp = compute_file_picker_layout(dlg, LINE_H, FP_SB_W);
                let _ = draw_file_picker(
                    &mut fb, &bevel, &layout_fp,
                    &fp.picker, &fp.filename.text,
                    &["All files", "Text files (*.txt)"],
                    fp.filetype_sel,
                    picker_mode == PickerMode::Load,
                    fp.focus,
                    LINE_H, CHAR_W, font, &theme.colors, &fp_sb,
                );
            }
        }

        // Blit
        drop(fb);
        {
            let mut fb_mem = gop.frame_buffer();
            let base = fb_mem.as_mut_ptr();
            let len  = fb_mem.size();
            let slice = unsafe { core::slice::from_raw_parts_mut(base, len) };
            if slice.len() == back.len() { slice.copy_from_slice(&back); }
        }
    }
}

// ── Action helpers ────────────────────────────────────────────────────────────

fn do_new(textarea: &mut TextArea, current_file: &mut Option<(Vec<String>, String)>) {
    *textarea = TextArea::from_str("");
    *current_file = None;
}

fn do_save(
    settings: &mut EditorSettings,
    current_file: &Option<(Vec<String>, String)>,
    textarea: &TextArea,
    last_dir: &[String],
    status: &mut String,
) {
    if let Some((ref path, ref name)) = current_file {
        let user = find_user_fs_handles();
        let all  = list_simple_fs_handles();
        if let Some(fh) = user.first().or_else(|| all.first()).copied() {
            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(fh) {
                let mut io = SimpleFsIo { fs: &mut *fs };
                if io.write_file(path, name, textarea.text.as_bytes()).is_ok() {
                    settings.last_file = name.clone();
                    settings.set_last_dir_path(last_dir);
                    save_settings(settings);
                    *status = format!("Saved: {}  Esc: menu", name);
                } else {
                    *status = format!("Error saving: {}  Esc: menu", name);
                }
            }
        }
    }
    // If no current_file, caller handles opening the picker
}
