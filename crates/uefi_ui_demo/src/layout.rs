//! Layout geometry shared by the UEFI binary and host snapshot renderer.

use alloc::vec::Vec;

use uefi_ui::embedded_graphics::geometry::Point;
use uefi_ui::embedded_graphics::prelude::*;
use uefi_ui::embedded_graphics::primitives::Rectangle;
use uefi_ui::layout::{pad, row_panels_fit_start};
use uefi_ui::popover::center_in_screen;
use uefi_ui::window::WindowOffset;

/// Focus regions (same semantics as the interactive firmware demo).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Menu,
    Editor,
    Scrollbar,
    Gallery,
}

pub const MENU_LABELS: &[&str] = &["&File", "&Edit", "&View", "&Help"];
pub static FILE_MENU: &[&str] = &["&New", "&Open…", "&Save", "—", "E&xit"];
pub static EDIT_MENU: &[&str] = &["&Undo", "Cu&t", "&Copy", "&Paste", "Select &All"];
pub static VIEW_MENU: &[&str] = &["&Refresh", "&Status bar"];
pub static HELP_MENU: &[&str] = &["&Contents", "&About"];
pub static SUBMENUS: &[Option<&[&str]>] = &[
    Some(FILE_MENU),
    Some(EDIT_MENU),
    Some(VIEW_MENU),
    Some(HELP_MENU),
];

pub const GALLERY_W: u32 = 300;
pub const MENU_CELL_PAD_LEFT: u32 = 4;
pub const MENU_CELL_PAD_RIGHT: u32 = 8;

/// Layout shared by hit-testing and painting.
pub struct UiLayout {
    pub win: Rectangle,
    pub title_bar: Rectangle,
    /// Full-width strip under the title bar (behind [`menu_cells`]).
    pub menu_strip: Rectangle,
    pub menu_cells: Vec<Rectangle>,
    pub preview_rect: Rectangle,
    pub preview_inner: Rectangle,
    pub gallery_rect: Rectangle,
    pub gallery_inner: Rectangle,
    pub text_rect: Rectangle,
    pub sb_rect: Rectangle,
    pub text_inner: Rectangle,
    pub visible_lines: usize,
    pub aux_panel: Rectangle,
}

pub fn compute_layout(
    w: usize,
    h: usize,
    line_h: i32,
    menu_char_w: u32,
    aux_nudge: WindowOffset,
) -> UiLayout {
    let screen = Rectangle::new(Point::zero(), Size::new(w as u32, h as u32));
    let win = center_in_screen(
        Size::new(
            (w * 4 / 5).max(480) as u32,
            (h * 4 / 5).max(360) as u32,
        ),
        screen,
    );
    const TITLE_H: u32 = 26;
    const MENU_H: u32 = 22;
    const PREVIEW_H: u32 = 128;
    let title_bar = Rectangle::new(win.top_left, Size::new(win.size.width, TITLE_H));
    let menu_strip = Rectangle::new(
        Point::new(win.top_left.x, win.top_left.y + TITLE_H as i32),
        Size::new(win.size.width, MENU_H),
    );
    let body = Rectangle::new(
        Point::new(
            win.top_left.x,
            win.top_left.y + TITLE_H as i32 + MENU_H as i32,
        ),
        Size::new(
            win.size.width,
            win.size.height.saturating_sub(TITLE_H + MENU_H),
        ),
    );
    let menu_widths: Vec<u32> = MENU_LABELS
        .iter()
        .map(|label| {
            let text_w = label.chars().filter(|&c| c != '&').count() as u32 * menu_char_w;
            MENU_CELL_PAD_LEFT + text_w + MENU_CELL_PAD_RIGHT
        })
        .collect();
    let menu_cells = row_panels_fit_start(pad(menu_strip, 2), menu_widths.as_slice(), 4);
    let body_pad = pad(body, 6);
    const SB_W: u32 = 26; // Bedrock: scrollbar = 26px
    let preview_h = PREVIEW_H.min(body_pad.size.height.saturating_sub(48));
    let preview_rect = Rectangle::new(
        body_pad.top_left,
        Size::new(body_pad.size.width, preview_h),
    );
    let preview_inner = pad(preview_rect, 4);
    let row_top = body_pad.top_left.y + preview_rect.size.height as i32 + 4;
    let row_h = body_pad
        .size
        .height
        .saturating_sub(preview_rect.size.height + 4);
    let gallery_rect = Rectangle::new(
        Point::new(body_pad.top_left.x, row_top),
        Size::new(GALLERY_W, row_h),
    );
    let gallery_inner = pad(gallery_rect, 3);
    let text_w = body_pad
        .size
        .width
        .saturating_sub(GALLERY_W + 6 + SB_W + 4);
    let text_rect = Rectangle::new(
        Point::new(body_pad.top_left.x + GALLERY_W as i32 + 6, row_top),
        Size::new(text_w, row_h),
    );
    let sb_rect = Rectangle::new(
        Point::new(
            text_rect.top_left.x + text_rect.size.width as i32 + 4,
            row_top,
        ),
        Size::new(SB_W, row_h),
    );
    let text_inner = pad(text_rect, 3);
    let visible_lines = (text_inner.size.height as i32 / line_h).max(1) as usize;
    const AUX_W: u32 = 192;
    const AUX_H: u32 = 100;
    let aux_panel = Rectangle::new(
        Point::new(
            body_pad.top_left.x + body_pad.size.width as i32 - AUX_W as i32 - 8 + aux_nudge.x,
            body_pad.top_left.y + body_pad.size.height as i32 - AUX_H as i32 - 8 + aux_nudge.y,
        ),
        Size::new(AUX_W, AUX_H),
    );
    UiLayout {
        win,
        title_bar,
        menu_strip,
        menu_cells,
        preview_rect,
        preview_inner,
        gallery_rect,
        gallery_inner,
        text_rect,
        sb_rect,
        text_inner,
        visible_lines,
        aux_panel,
    }
}

/// Dropdown under a menubar cell (`labels` = submenu entries).
pub fn submenu_popup_rect(cell: &Rectangle, labels: &[&str], char_w: i32, line_h: i32) -> Rectangle {
    let max_chars = labels.iter().map(|s| s.chars().filter(|&c| c != '&').count()).max().unwrap_or(0);
    let w = (max_chars as i32 * char_w + 16).max(72) as u32;
    let h = (labels.len() as i32 * line_h + 8).max(1) as u32;
    Rectangle::new(
        Point::new(cell.top_left.x, cell.top_left.y + cell.size.height as i32),
        Size::new(w, h),
    )
}
