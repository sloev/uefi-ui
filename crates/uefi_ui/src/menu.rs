//! Horizontal menu bar with wrapped focus (navbar-style).

/// Labels are borrowed for the frame; store static strings or arena-backed text in the app.
#[derive(Debug, Clone)]
pub struct MenuBar<'a> {
    labels: &'a [&'a str],
    focused: usize,
}

impl<'a> MenuBar<'a> {
    pub fn new(labels: &'a [&'a str]) -> Self {
        Self { labels, focused: 0 }
    }

    pub fn len(&self) -> usize {
        self.labels.len()
    }

    pub fn is_empty(&self) -> bool {
        self.labels.is_empty()
    }

    pub fn focused_index(&self) -> usize {
        self.focused
    }

    /// Focus a specific item (e.g. after a mouse hit-test).
    pub fn set_focused_index(&mut self, index: usize) {
        if !self.labels.is_empty() && index < self.labels.len() {
            self.focused = index;
        }
    }

    pub fn label(&self, index: usize) -> Option<&'a str> {
        self.labels.get(index).copied()
    }

    pub fn nav_next(&mut self) {
        let n = self.labels.len();
        if n == 0 {
            return;
        }
        self.focused = (self.focused + 1) % n;
    }

    pub fn nav_prev(&mut self) {
        let n = self.labels.len();
        if n == 0 {
            return;
        }
        self.focused = self.focused.checked_sub(1).unwrap_or(n - 1);
    }

    /// “Enter” on the focused item: returns its index.
    pub fn activate_focused(&self) -> Option<usize> {
        if self.labels.is_empty() {
            None
        } else {
            Some(self.focused)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s3_nav_next_wraps() {
        let labels: &[&str] = &["A", "B", "C"];
        let mut m = MenuBar::new(labels);
        assert_eq!(m.focused_index(), 0);
        m.nav_next();
        assert_eq!(m.focused_index(), 1);
        m.nav_next();
        m.nav_next();
        assert_eq!(m.focused_index(), 0);
    }

    #[test]
    fn s3_nav_prev_wraps() {
        let labels: &[&str] = &["A", "B"];
        let mut m = MenuBar::new(labels);
        m.nav_prev();
        assert_eq!(m.focused_index(), 1);
        m.nav_prev();
        assert_eq!(m.focused_index(), 0);
    }

    #[test]
    fn s3_activate_focused_returns_index() {
        let labels: &[&str] = &["File", "Edit"];
        let mut m = MenuBar::new(labels);
        m.nav_next();
        assert_eq!(m.activate_focused(), Some(1));
    }
}
