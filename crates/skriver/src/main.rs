//! **skriver** -- bootable UEFI text editor.
//!
//! Boots directly to a full-screen editor. File picker is rooted to user-accessible
//! USB/removable volumes (EFI partitions are hidden). Settings (font size, theme,
//! last file, last directory) are persisted in UEFI non-volatile RAM between boots.
#![no_main]
#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use uefi::boot::{
    self, open_protocol_exclusive, EventType, TimerTrigger, Tpl,
};
use uefi::prelude::*;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};
use uefi::proto::console::text::{Input, Key as UefiKey, ScanCode};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;

use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::bedrock_controls::{
    compute_file_picker_layout, draw_file_picker, draw_menu_bar, draw_menu_popup,
    draw_window_title_bar, FP_SB_W,
};
use uefi_ui::editor_settings::EditorSettings;
use uefi_ui::embedded_graphics::mono_font::ascii::FONT_6X10;
use uefi_ui::embedded_graphics::mono_font::MonoTextStyle;
use uefi_ui::embedded_graphics::prelude::*;
use uefi_ui::embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use uefi_ui::embedded_graphics::text::{Baseline, Text};
use uefi_ui::embedded_graphics::geometry::{Point, Size};
use uefi_ui::file_picker::{FileIo, FilePickerDialogAction, FilePickerDialogState, PickerMode};
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::input::{Key, KeyEvent, Modifiers};
use uefi_ui::theme::Theme;
use uefi_ui::uefi_vars::{load_settings_blob, save_settings_blob};
use uefi_ui::{find_user_fs_handles, list_simple_fs_handles, SimpleFsIo};
use uefi_ui::widgets::{textarea_sync_vertical_scroll, ScrollAxis, ScrollbarState, TextArea};

// ── Constants ────────────────────────────────────────────────────────────────
const TITLE_H: i32 = 20;
const MENU_H:  i32 = 20;
const SB_W:    i32 = 18;
const LINE_H:  i32 = 10;
const CHAR_W:  i32 = 6;

// ── Menus ────────────────────────────────────────────────────────────────────
const MENU_LABELS: &[&str] = &["&File"];
const FILE_MENU:   &[&str] = &["&New", "&Open...", "&Save", "Save &As...", "--",
                                "&Delete file", "--", "E&xit"];

fn settings_var_name() -> CString16 {
    CString16::try_from("SkriverSettings").unwrap_or_else(|_| CString16::try_from("S").unwrap())
}

fn load_settings() -> EditorSettings {
    let name = settings_var_name();
    load_settings_blob(&name)
        .ok()
        .flatten()
        .and_then(|b| EditorSettings::from_blob(&b))
        .unwrap_or_default()
}

fn save_settings(s: &EditorSettings) {
    let name = settings_var_name();
    let _ = save_settings_blob(&name, &s.to_blob());
}

// ── Key mapping ──────────────────────────────────────────────────────────────
fn map_key(k: UefiKey) -> Option<KeyEvent> {
    match k {
        UefiKey::Special(ScanCode::LEFT)      => Some(KeyEvent::new(Key::Left)),
        UefiKey::Special(ScanCode::RIGHT)     => Some(KeyEvent::new(Key::Right)),
        UefiKey::Special(ScanCode::UP)        => Some(KeyEvent::new(Key::Up)),
        UefiKey::Special(ScanCode::DOWN)      => Some(KeyEvent::new(Key::Down)),
        UefiKey::Special(ScanCode::HOME)      => Some(KeyEvent::new(Key::Home)),
        UefiKey::Special(ScanCode::END)       => Some(KeyEvent::new(Key::End)),
        UefiKey::Special(ScanCode::DELETE)    => Some(KeyEvent::new(Key::Delete)),
        UefiKey::Special(ScanCode::PAGE_UP)   => Some(KeyEvent::new(Key::PageUp)),
        UefiKey::Special(ScanCode::PAGE_DOWN) => Some(KeyEvent::new(Key::PageDown)),
        UefiKey::Special(ScanCode::ESCAPE)    => Some(KeyEvent::new(Key::Escape)),
        UefiKey::Special(ScanCode::FUNCTION_10) => Some(KeyEvent::new(Key::Function(10))),
        UefiKey::Printable(ch) => {
            let c: char = ch.into();
            if c == '\r' || c == '\n' { return Some(KeyEvent::new(Key::Enter)); }
            if c == '\t'              { return Some(KeyEvent::new(Key::Tab)); }
            if c == '\u{8}'           { return Some(KeyEvent::new(Key::Backspace)); }
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

// ── Scrollbar sync ────────────────────────────────────────────────────────────
fn scroll_keys(ta: &mut TextArea, sb: &mut ScrollbarState, ev: &KeyEvent, vl: usize) {
    let vl = vl.max(1);
    let max_top = ta.line_count().saturating_sub(vl);
    match ev.key {
        Key::Up       => { ta.scroll_top_line = ta.scroll_top_line.saturating_sub(1); }
        Key::Down     => { ta.scroll_top_line = (ta.scroll_top_line + 1).min(max_top); }
        Key::PageUp   => { ta.scroll_top_line = ta.scroll_top_line.saturating_sub(vl); }
        Key::PageDown => { ta.scroll_top_line = (ta.scroll_top_line + vl).min(max_top); }
        Key::Home     => { ta.scroll_top_line = 0; }
        Key::End      => { ta.scroll_top_line = max_top; }
        _ => {}
    }
    ta.scroll_top_line = ta.scroll_top_line.min(max_top);
    textarea_sync_vertical_scroll(ta, vl, 0, sb);
}

// ── Compute popup rect under a menu cell ─────────────────────────────────────
fn popup_rect_for(cell: &Rectangle, items: &[&str]) -> Rectangle {
    let max_ch = items.iter()
        .map(|s| s.chars().filter(|&c| c != '&' && c != '-').count())
        .max().unwrap_or(0) as i32;
    let pw = (max_ch * CHAR_W + 20).max(72) as u32;
    let ph = (items.len() as i32 * LINE_H + 8).max(4) as u32;
    Rectangle::new(
        Point::new(cell.top_left.x, cell.top_left.y + cell.size.height as i32),
        Size::new(pw, ph),
    )
}

// ── Open picker helper ────────────────────────────────────────────────────────
fn open_picker(mode: PickerMode, last_dir: &[String]) -> Option<FilePickerDialogState> {
    let user = find_user_fs_handles();
    let all  = list_simple_fs_handles();
    let h = user.first().or_else(|| all.first()).copied()?;
    let mut fs = open_protocol_exclusive::<SimpleFileSystem>(h).ok()?;
    let mut io = SimpleFsIo { fs: &mut *fs };
    FilePickerDialogState::with_root(mode, 2, alloc::vec![], last_dir.to_vec(), &mut io).ok()
}

// ── Entry point ───────────────────────────────────────────────────────────────
#[entry]
fn main() -> Status {
    uefi::helpers::init().unwrap();

    // Enumerate and connect all handles so USB HID drivers bind
    if let Ok(handles) = boot::locate_handle_buffer(uefi::boot::SearchType::AllHandles) {
        for &h in handles.iter() { let _ = boot::connect_controller(h, None, None, true); }
    }

    // ── GOP ─────────────────────────────────────────────────────────────────
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().unwrap();
    let mut gop = open_protocol_exclusive::<GraphicsOutput>(gop_handle).unwrap();
    let info = gop.current_mode_info();
    if info.pixel_format() != PixelFormat::Bgr {
        uefi::println!("[skriver] pixel format {:?} unsupported", info.pixel_format());
        return Status::UNSUPPORTED;
    }
    let (w, h) = info.resolution();
    let stride_bytes = info.stride() * 4;
    let fb_size = gop.frame_buffer().size();

    // ── Back buffer ──────────────────────────────────────────────────────────
    let mut back: Vec<u8> = Vec::new();
    if back.try_reserve_exact(fb_size).is_err() { return Status::OUT_OF_RESOURCES; }
    back.resize(fb_size, 0);

    // ── Settings ─────────────────────────────────────────────────────────────
    let mut settings = load_settings();

    // ── Theme + bevel ────────────────────────────────────────────────────────
    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;
    let font  = &FONT_6X10;

    // ── Layout geometry ──────────────────────────────────────────────────────
    let status_h = LINE_H + 4;
    let editor_top = TITLE_H + MENU_H;
    let editor_bot = h as i32 - status_h;
    let editor_h   = (editor_bot - editor_top).max(0) as u32;
    let visible_lines = (editor_h as i32 / LINE_H).max(0) as usize;

    let title_rect  = Rectangle::new(Point::zero(), Size::new(w as u32, TITLE_H as u32));
    let menu_strip  = Rectangle::new(Point::new(0, TITLE_H), Size::new(w as u32, MENU_H as u32));
    let editor_rect = Rectangle::new(Point::new(0, editor_top), Size::new(w as u32, editor_h));
    let sb_rect     = Rectangle::new(Point::new(w as i32 - SB_W, editor_top), Size::new(SB_W as u32, editor_h));
    let inner_rect  = Rectangle::new(
        Point::new(1, editor_top + 1),
        Size::new((w as i32 - SB_W - 2).max(0) as u32, editor_h.saturating_sub(2)),
    );
    let status_rect = Rectangle::new(Point::new(0, editor_bot), Size::new(w as u32, status_h as u32));

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

    // ── Editor state ──────────────────────────────────────────────────────────
    let init_text = if !settings.last_file.is_empty() {
        format!("Last file: {}  (Ctrl+O to reopen)", settings.last_file)
    } else {
        String::from("")
    };
    let mut textarea = TextArea::from_str(&init_text);
    let mut sb = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);
    let mut current_file: Option<(Vec<String>, String)> = None;
    let mut last_dir = settings.last_dir_path();
    let mut picker_mode = PickerMode::Load;

    // ── File picker state ────────────────────────────────────────────────────
    let mut file_picker: Option<FilePickerDialogState> = None;
    let mut fp_sb = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);

    // ── Menu state ────────────────────────────────────────────────────────────
    let mut menu_open: Option<usize> = None;
    let mut menu_sel: usize = 0;

    // ── Status message ────────────────────────────────────────────────────────
    let mut status: String = String::from("Ctrl+O Open  Ctrl+S Save  Ctrl+N New  Ctrl+Q Quit  F10 Menu");

    // ── Keyboard input ────────────────────────────────────────────────────────
    let stdin = boot::get_handle_for_protocol::<Input>().unwrap();
    let mut kb = open_protocol_exclusive::<Input>(stdin).unwrap();

    // ── Timer ─────────────────────────────────────────────────────────────────
    let timer = unsafe {
        boot::create_event(EventType::TIMER, Tpl::APPLICATION, None, None).unwrap()
    };
    boot::set_timer(&timer, TimerTrigger::Periodic(166_666)).unwrap();

    uefi::println!("[skriver] {}x{} ready", w, h);

    let mut dirty = true;

    loop {
        let mut events = [kb.wait_for_key_event().unwrap(), unsafe { timer.unsafe_clone() }];
        let _ = boot::wait_for_event(&mut events);

        // ── Read keys ────────────────────────────────────────────────────────
        while let Ok(Some(raw)) = kb.read_key() {
            let Some(ev) = map_key(raw) else { continue };

            // ── File picker: route all keys here ─────────────────────────────
            if file_picker.is_some() {
                let mut fp = file_picker.take().unwrap();
                let user = find_user_fs_handles();
                let all  = list_simple_fs_handles();
                let action = if let Some(h) = user.first().or_else(|| all.first()).copied() {
                    if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                        let mut io = SimpleFsIo { fs: &mut *fs };
                        fp.handle_key(&ev, visible_lines, &mut io)
                            .unwrap_or(FilePickerDialogAction::None)
                    } else { FilePickerDialogAction::Cancel }
                } else { FilePickerDialogAction::Cancel };

                match action {
                    FilePickerDialogAction::Confirm { path, name } => {
                        last_dir = fp.last_dir.clone();
                        let user2 = find_user_fs_handles();
                        let all2  = list_simple_fs_handles();
                        if let Some(h) = user2.first().or_else(|| all2.first()).copied() {
                            if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                let mut io = SimpleFsIo { fs: &mut *fs };
                                if picker_mode == PickerMode::Load {
                                    match io.read_file(&path, &name) {
                                        Ok(bytes) => {
                                            let text = core::str::from_utf8(&bytes).unwrap_or("[binary]");
                                            textarea = TextArea::from_str(text);
                                            current_file = Some((path, name.clone()));
                                            settings.last_file = name.clone();
                                            settings.set_last_dir_path(&last_dir);
                                            save_settings(&settings);
                                            status = format!("Opened: {}", name);
                                        }
                                        Err(_) => { status = format!("Error reading: {}", name); }
                                    }
                                } else {
                                    let text = &textarea.text;
                                    match io.write_file(&path, &name, text.as_bytes()) {
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
                    }
                    FilePickerDialogAction::Cancel => {
                        last_dir = fp.last_dir.clone();
                        status = String::from("Cancelled.");
                    }
                    FilePickerDialogAction::None => {
                        // Keep picker open
                        let n = fp.picker.entries.len().max(1);
                        fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, visible_lines.max(1), fp.picker.scroll_top);
                        file_picker = Some(fp);
                    }
                }
                dirty = true;
                continue;
            }

            // ── Escape: close menu ────────────────────────────────────────────
            if ev.key == Key::Escape {
                menu_open = None;
                dirty = true;
                continue;
            }

            // ── Ctrl shortcuts ────────────────────────────────────────────────
            if ev.modifiers.ctrl {
                match ev.key {
                    Key::Character('n') | Key::Character('N') => {
                        textarea = TextArea::from_str("");
                        current_file = None;
                        status = String::from("New file.");
                    }
                    Key::Character('o') | Key::Character('O') => {
                        picker_mode = PickerMode::Load;
                        match open_picker(PickerMode::Load, &last_dir) {
                            Some(fp) => {
                                let n = fp.picker.entries.len().max(1);
                                fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, visible_lines.max(1), 0);
                                file_picker = Some(fp);
                            }
                            None => { status = String::from("No accessible volume found."); }
                        }
                    }
                    Key::Character('s') | Key::Character('S') => {
                        if let Some((ref path, ref name)) = current_file.clone() {
                            let user = find_user_fs_handles();
                            let all  = list_simple_fs_handles();
                            if let Some(h) = user.first().or_else(|| all.first()).copied() {
                                if let Ok(mut fs) = open_protocol_exclusive::<SimpleFileSystem>(h) {
                                    let mut io = SimpleFsIo { fs: &mut *fs };
                                    let text = &textarea.text;
                                    if io.write_file(path, name, text.as_bytes()).is_ok() {
                                        settings.last_file = name.clone();
                                        settings.set_last_dir_path(&last_dir);
                                        save_settings(&settings);
                                        status = format!("Saved: {}", name);
                                    } else {
                                        status = format!("Error saving: {}", name);
                                    }
                                }
                            }
                        } else {
                            picker_mode = PickerMode::Save;
                            match open_picker(PickerMode::Save, &last_dir) {
                                Some(fp) => {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, visible_lines.max(1), 0);
                                    file_picker = Some(fp);
                                }
                                None => { status = String::from("No accessible volume found."); }
                            }
                        }
                    }
                    Key::Character('q') | Key::Character('Q') => {
                        settings.set_last_dir_path(&last_dir);
                        save_settings(&settings);
                        return Status::SUCCESS;
                    }
                    _ => { textarea.apply_key_event(&ev); }
                }
                dirty = true;
                continue;
            }

            // ── F10: open File menu ───────────────────────────────────────────
            if ev.key == Key::Function(10) {
                menu_open = Some(0);
                menu_sel = 0;
                dirty = true;
                continue;
            }

            // ── Menu navigation ───────────────────────────────────────────────
            if let Some(_mi) = menu_open {
                match ev.key {
                    Key::Escape => { menu_open = None; }
                    Key::Up     => { if menu_sel > 0 { menu_sel -= 1; } }
                    Key::Down   => { if menu_sel + 1 < FILE_MENU.len() { menu_sel += 1; } }
                    Key::Enter  => {
                        match menu_sel {
                            0 => { // New
                                textarea = TextArea::from_str("");
                                current_file = None;
                                status = String::from("New file.");
                            }
                            1 => { // Open
                                picker_mode = PickerMode::Load;
                                if let Some(fp) = open_picker(PickerMode::Load, &last_dir) {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, visible_lines.max(1), 0);
                                    file_picker = Some(fp);
                                }
                            }
                            2 | 3 => { // Save / Save As
                                picker_mode = PickerMode::Save;
                                if let Some(fp) = open_picker(PickerMode::Save, &last_dir) {
                                    let n = fp.picker.entries.len().max(1);
                                    fp_sb = ScrollbarState::new(ScrollAxis::Vertical, n, visible_lines.max(1), 0);
                                    file_picker = Some(fp);
                                }
                            }
                            5 => { // Delete
                                status = String::from("Delete not yet implemented.");
                            }
                            7 => { // Exit
                                settings.set_last_dir_path(&last_dir);
                                save_settings(&settings);
                                return Status::SUCCESS;
                            }
                            _ => {}
                        }
                        menu_open = None;
                    }
                    _ => {}
                }
                dirty = true;
                continue;
            }

            // ── Editor ───────────────────────────────────────────────────────
            textarea.apply_key_event(&ev);
            let total = textarea.line_count().max(1);
            sb = ScrollbarState::new(ScrollAxis::Vertical, total, visible_lines.max(1), textarea.scroll_top_line);
            dirty = true;
        }

        if !dirty { continue; }
        dirty = false;

        // ── Paint scene ────────────────────────────────────────────────────────
        let Some(mut fb) = BgrxFramebuffer::new(&mut back, w as u32, h as u32, stride_bytes)
        else { return Status::DEVICE_ERROR; };

        // Background
        fb.fill_rect_solid(0, 0, w as u32, h as u32, theme.colors.background);

        // Title bar
        let title_text = match &current_file {
            Some((_, n)) => format!("skriver -- {}", n),
            None => String::from("skriver -- [untitled]"),
        };
        let _ = draw_window_title_bar(
            &mut fb, title_rect, &title_text, font, true,
            theme.colors.accent, theme.colors.caption_on_accent, theme.colors.border,
        );

        // Menu bar
        let _ = draw_menu_bar(
            &mut fb, menu_strip, &menu_cells, MENU_LABELS,
            menu_open, font,
            theme.colors.surface, theme.colors.text, theme.colors.accent, theme.colors.caption_on_accent,
        );

        // Editor area: sunken border
        let _ = bevel.draw_sunken(&mut fb, editor_rect);

        // Fill editor background
        Rectangle::new(inner_rect.top_left, inner_rect.size)
            .into_styled(PrimitiveStyle::with_fill(theme.colors.canvas))
            .draw(&mut fb)
            .ok();

        // Text lines
        let lines = textarea.lines();
        for (li, line) in lines.iter()
            .enumerate()
            .skip(textarea.scroll_top_line)
            .take(visible_lines)
        {
            let vy = li - textarea.scroll_top_line;
            let y  = inner_rect.top_left.y + vy as i32 * LINE_H;
            let x  = inner_rect.top_left.x + 2;
            let style = MonoTextStyle::new(font, theme.colors.text);
            let _ = Text::with_baseline(line, Point::new(x, y + 1), style, Baseline::Top).draw(&mut fb);
        }

        // Scrollbar area
        let _ = bevel.draw_sunken(&mut fb, sb_rect);

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

        // Menu popup
        if let Some(mi) = menu_open {
            if let Some(cell) = menu_cells.get(mi) {
                let popup = popup_rect_for(cell, FILE_MENU);
                let _ = draw_menu_popup(
                    &mut fb, &bevel, popup, FILE_MENU, menu_sel,
                    LINE_H, font,
                    theme.colors.canvas, theme.colors.text,
                    theme.colors.accent, theme.colors.caption_on_accent,
                );
            }
        }

        // File picker overlay
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

        // ── Blit back buffer to GOP framebuffer ───────────────────────────────
        drop(fb);
        {
            let mut fb_mem = gop.frame_buffer();
            let base = fb_mem.as_mut_ptr();
            let len  = fb_mem.size();
            let slice = unsafe { core::slice::from_raw_parts_mut(base, len) };
            if slice.len() == back.len() {
                slice.copy_from_slice(&back);
            }
        }
    }
}
