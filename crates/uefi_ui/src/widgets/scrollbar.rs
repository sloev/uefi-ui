//! Scrollbar thumb position (pair with [`super::TextArea`] or any scrollable content).
//!
//! ## Hit-testing (S16.2)
//!
//! Call [`ScrollbarState::hit_test_vertical`] with the scrollbar rectangle and a pointer position
//! to obtain a [`ScrollbarHit`].  Apply the result with the convenience scroll methods:
//!
//! ```text
//! match sb.hit_test_vertical(sb_rect, px, py) {
//!     ScrollbarHit::ArrowUp   => sb.scroll_line_up(),
//!     ScrollbarHit::ArrowDown => sb.scroll_line_down(),
//!     ScrollbarHit::PageUp    => sb.scroll_page_up(),
//!     ScrollbarHit::PageDown  => sb.scroll_page_down(),
//!     ScrollbarHit::Thumb     => { /* begin drag — call set_offset_from_ratio each frame */ }
//!     ScrollbarHit::None      => {}
//! }
//! ```

/// Scrollbar orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// Result of [`ScrollbarState::hit_test_vertical`] — which zone of the scrollbar was hit.
///
/// S16.2: arrows (line scroll), track above/below thumb (page), thumb drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHit {
    /// Pointer is on the up-arrow button.
    ArrowUp,
    /// Pointer is on the down-arrow button.
    ArrowDown,
    /// Pointer is on the track above the thumb (page up zone).
    PageUp,
    /// Pointer is on the track below the thumb (page down zone).
    PageDown,
    /// Pointer is on the thumb itself (begin drag).
    Thumb,
    /// Pointer outside the scrollbar rectangle.
    None,
}

/// Normalized scroll state: `thumb_center` is 0..=1 when content overflows.
#[derive(Debug, Clone)]
pub struct ScrollbarState {
    pub axis: ScrollAxis,
    /// Total content extent in abstract units (e.g. lines or px).
    pub content_len: usize,
    /// Visible viewport (same units).
    pub viewport_len: usize,
    /// Scroll offset: first visible unit index.
    pub offset: usize,
}

impl ScrollbarState {
    pub fn new(axis: ScrollAxis, content_len: usize, viewport_len: usize, offset: usize) -> Self {
        Self {
            axis,
            content_len,
            viewport_len,
            offset,
        }
    }

    /// `None` if there is nothing to scroll.
    pub fn thumb_center_ratio(&self) -> Option<f32> {
        if self.content_len <= self.viewport_len || self.viewport_len == 0 {
            return None;
        }
        let max_off = self.content_len - self.viewport_len;
        let t = self.offset as f32 / max_off as f32;
        Some(t.clamp(0.0, 1.0))
    }

    /// Thumb length ratio along the track (0..1).
    pub fn thumb_length_ratio(&self) -> Option<f32> {
        if self.content_len == 0 || self.viewport_len == 0 {
            return None;
        }
        if self.content_len <= self.viewport_len {
            return None;
        }
        Some((self.viewport_len as f32 / self.content_len as f32).clamp(0.05, 1.0))
    }

    /// Set offset from clicking/dragging at `ratio` 0..=1 along the track.
    pub fn set_offset_from_ratio(&mut self, ratio: f32) {
        if self.content_len <= self.viewport_len {
            self.offset = 0;
            return;
        }
        let max_off = self.content_len - self.viewport_len;
        self.offset = ((ratio.clamp(0.0, 1.0)) * max_off as f32 + 0.5) as usize;
    }

    /// Scroll up by one line; clamps at 0.
    pub fn scroll_line_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
    }

    /// Scroll down by one line; clamps at max offset.
    pub fn scroll_line_down(&mut self) {
        if self.content_len > self.viewport_len {
            let max_off = self.content_len - self.viewport_len;
            self.offset = (self.offset + 1).min(max_off);
        }
    }

    /// Scroll up by one viewport page; clamps at 0.
    pub fn scroll_page_up(&mut self) {
        self.offset = self.offset.saturating_sub(self.viewport_len.max(1));
    }

    /// Scroll down by one viewport page; clamps at max offset.
    pub fn scroll_page_down(&mut self) {
        if self.content_len > self.viewport_len {
            let max_off = self.content_len - self.viewport_len;
            self.offset = (self.offset + self.viewport_len.max(1)).min(max_off);
        }
    }

    /// S16.2 — Hit-test a pointer at `(px, py)` against a **vertical** scrollbar drawn at `rect`.
    ///
    /// The geometry mirrors [`crate::bedrock_controls::draw_scrollbar_vertical`]:
    /// - Arrow buttons are square (`arrow_h = rect.width`).
    /// - Track occupies the remaining height between the two arrows.
    /// - Thumb position and length are derived from the current scroll state.
    pub fn hit_test_vertical(
        &self,
        rect_x: i32,
        rect_y: i32,
        rect_w: u32,
        rect_h: u32,
        px: i32,
        py: i32,
    ) -> ScrollbarHit {
        // Outside bounding box.
        if px < rect_x || px >= rect_x + rect_w as i32 || py < rect_y || py >= rect_y + rect_h as i32 {
            return ScrollbarHit::None;
        }
        let arrow_h = rect_w as i32; // square buttons
        // Up arrow
        if py < rect_y + arrow_h {
            return ScrollbarHit::ArrowUp;
        }
        // Down arrow
        if py >= rect_y + rect_h as i32 - arrow_h {
            return ScrollbarHit::ArrowDown;
        }
        // Track area
        let track_y = rect_y + arrow_h;
        let track_h = (rect_h as i32 - arrow_h * 2).max(0) as u32;
        if track_h == 0 {
            return ScrollbarHit::None;
        }
        let (thumb_y, thumb_h) = self.thumb_geometry(track_y, track_h);
        if py >= thumb_y && py < thumb_y + thumb_h as i32 {
            return ScrollbarHit::Thumb;
        }
        if py < thumb_y {
            ScrollbarHit::PageUp
        } else {
            ScrollbarHit::PageDown
        }
    }

    /// Convert an absolute `py` within the track to a scroll ratio (0..=1) for thumb drag.
    ///
    /// `drag_anchor_offset` is the offset in pixels from the top of the thumb where the
    /// drag started (so the thumb follows the pointer without jumping).  Pass `0` to snap
    /// the thumb top to the pointer.
    pub fn drag_ratio_from_y(
        &self,
        rect_y: i32,
        rect_w: u32,
        rect_h: u32,
        py: i32,
        drag_anchor_offset: i32,
    ) -> f32 {
        let arrow_h = rect_w as i32;
        let track_y = rect_y + arrow_h;
        let track_h = (rect_h as i32 - arrow_h * 2).max(0) as u32;
        if track_h == 0 {
            return 0.0;
        }
        let (_, thumb_h) = self.thumb_geometry(track_y, track_h);
        let avail = track_h.saturating_sub(thumb_h) as f32;
        if avail <= 0.0 {
            return 0.0;
        }
        let thumb_top_target = py - drag_anchor_offset - track_y;
        (thumb_top_target as f32 / avail).clamp(0.0, 1.0)
    }

    /// Thumb top-y and pixel height within the track — mirrors `draw_scrollbar_vertical`.
    fn thumb_geometry(&self, track_y: i32, track_h: u32) -> (i32, u32) {
        let tc = match self.thumb_center_ratio() {
            Some(v) => v,
            None => return (track_y, track_h),
        };
        let tl = match self.thumb_length_ratio() {
            Some(v) => v,
            None => return (track_y, track_h),
        };
        let th = ((track_h as f32 * tl) as u32).max(12).min(track_h);
        let avail = track_h.saturating_sub(th);
        let ty = track_y + (avail as f32 * tc.clamp(0.0, 1.0)) as i32;
        (ty, th)
    }
}

/// Sync vertical scrollbar from a textarea’s line scroll.
///
/// Pass `cols = 0` for unwrapped (logical line) sync, or the viewport column count for S15 wrapped sync.
#[allow(dead_code)] // Public API for firmware; not called from other modules in this crate.
pub fn textarea_sync_vertical_scroll(
    textarea: &super::TextArea,
    visible_lines: usize,
    cols: usize,
    scroll: &mut ScrollbarState,
) {
    let lines = if cols > 0 {
        textarea.wrapped_line_count(cols)
    } else {
        textarea.line_count()
    }.max(1);
    scroll.axis = ScrollAxis::Vertical;
    scroll.content_len = lines;
    scroll.viewport_len = visible_lines.max(1);
    scroll.offset = textarea.scroll_top_line;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::TextArea;

    #[test]
    fn textarea_sync_smoke() {
        let ta = TextArea::from_str("a\nb");
        let mut sb = ScrollbarState::new(ScrollAxis::Vertical, 0, 1, 0);
        textarea_sync_vertical_scroll(&ta, 1, 0, &mut sb);
        assert_eq!(sb.offset, ta.scroll_top_line);
    }

    #[test]
    fn thumb_ratios() {
        let s = ScrollbarState::new(ScrollAxis::Vertical, 100, 10, 45);
        assert!((s.thumb_center_ratio().unwrap() - 0.5).abs() < 0.01);
        assert!(s.thumb_length_ratio().unwrap() < 1.0);
    }

    #[test]
    fn scroll_line_clamps() {
        let mut s = ScrollbarState::new(ScrollAxis::Vertical, 10, 3, 0);
        s.scroll_line_up(); // already at 0
        assert_eq!(s.offset, 0);
        s.offset = 7;
        s.scroll_line_down();
        assert_eq!(s.offset, 7); // max_off = 10-3 = 7
        s.scroll_line_down();
        assert_eq!(s.offset, 7);
    }

    #[test]
    fn scroll_page() {
        let mut s = ScrollbarState::new(ScrollAxis::Vertical, 20, 5, 0);
        s.scroll_page_down();
        assert_eq!(s.offset, 5);
        s.scroll_page_down();
        assert_eq!(s.offset, 10);
        s.scroll_page_up();
        assert_eq!(s.offset, 5);
    }

    #[test]
    fn hit_test_arrows() {
        // rect: x=0, y=0, w=26, h=200
        let s = ScrollbarState::new(ScrollAxis::Vertical, 100, 10, 0);
        assert_eq!(s.hit_test_vertical(0, 0, 26, 200, 13, 5), ScrollbarHit::ArrowUp);
        assert_eq!(s.hit_test_vertical(0, 0, 26, 200, 13, 195), ScrollbarHit::ArrowDown);
    }

    #[test]
    fn hit_test_outside() {
        let s = ScrollbarState::new(ScrollAxis::Vertical, 100, 10, 0);
        assert_eq!(s.hit_test_vertical(10, 10, 26, 200, 5, 50), ScrollbarHit::None);
    }

    #[test]
    fn hit_test_page_zones() {
        // 100 lines, 10 visible, offset=0 → thumb near top of track
        let s = ScrollbarState::new(ScrollAxis::Vertical, 100, 10, 0);
        // well below the thumb (which is near the top) should be PageDown
        assert_eq!(s.hit_test_vertical(0, 0, 26, 200, 13, 150), ScrollbarHit::PageDown);
    }
}
