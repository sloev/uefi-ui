//! Scroll area: viewport into a larger virtual surface (like egui `ScrollArea`).

/// Tracks 2D scroll offset and content extents (you clip when drawing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollArea {
    pub scroll_x: u32,
    pub scroll_y: u32,
    pub content_w: u32,
    pub content_h: u32,
    pub viewport_w: u32,
    pub viewport_h: u32,
}

impl ScrollArea {
    pub fn new(viewport_w: u32, viewport_h: u32, content_w: u32, content_h: u32) -> Self {
        Self {
            scroll_x: 0,
            scroll_y: 0,
            content_w,
            content_h,
            viewport_w,
            viewport_h,
        }
    }

    pub fn max_scroll_x(&self) -> u32 {
        self.content_w.saturating_sub(self.viewport_w)
    }

    pub fn max_scroll_y(&self) -> u32 {
        self.content_h.saturating_sub(self.viewport_h)
    }

    pub fn scroll_by(&mut self, dx: i32, dy: i32) {
        self.scroll_x = (self.scroll_x as i32 + dx)
            .clamp(0, self.max_scroll_x() as i32) as u32;
        self.scroll_y = (self.scroll_y as i32 + dy)
            .clamp(0, self.max_scroll_y() as i32) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_scroll_when_content_fits() {
        let a = ScrollArea::new(100, 50, 80, 40);
        assert_eq!(a.max_scroll_x(), 0);
        assert_eq!(a.max_scroll_y(), 0);
    }

    #[test]
    fn max_scroll_when_larger_than_viewport() {
        let a = ScrollArea::new(100, 50, 300, 200);
        assert_eq!(a.max_scroll_x(), 200);
        assert_eq!(a.max_scroll_y(), 150);
    }

    #[test]
    fn scroll_by_clamps() {
        let mut a = ScrollArea::new(10, 10, 100, 100);
        a.scroll_by(1000, 1000);
        assert_eq!(a.scroll_x, 90);
        assert_eq!(a.scroll_y, 90);
        a.scroll_by(-200, -200);
        assert_eq!(a.scroll_x, 0);
        assert_eq!(a.scroll_y, 0);
    }
}
