//! Scrollable single-select list (keyboard + optional pointer).

use crate::input::{Key, KeyEvent};

#[derive(Debug, Clone)]
pub struct ListBox<'a> {
    pub items: &'a [&'a str],
    pub selected: usize,
    /// First visible row index (for scrolling long lists).
    pub scroll_top: usize,
}

impl<'a> ListBox<'a> {
    pub fn new(items: &'a [&'a str]) -> Self {
        let n = items.len();
        Self {
            items,
            selected: if n == 0 { 0 } else { 0 },
            scroll_top: 0,
        }
    }

    pub fn visible_len(&self, rows: usize) -> usize {
        rows.min(self.items.len().saturating_sub(self.scroll_top))
    }

    pub fn apply_key(&mut self, key: Key, visible_rows: usize) {
        self.apply_key_event(&KeyEvent::new(key), visible_rows);
    }

    pub fn apply_key_event(&mut self, ev: &KeyEvent, visible_rows: usize) {
        let key = ev.key;
        let n = self.items.len();
        if n == 0 {
            return;
        }
        match key {
            Key::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                if self.selected < self.scroll_top {
                    self.scroll_top = self.selected;
                }
            }
            Key::Down => {
                if self.selected + 1 < n {
                    self.selected += 1;
                }
                let max_top = n.saturating_sub(visible_rows.max(1));
                if self.selected >= self.scroll_top + visible_rows.max(1) {
                    self.scroll_top = (self.selected + 1).saturating_sub(visible_rows.max(1));
                    if self.scroll_top > max_top {
                        self.scroll_top = max_top;
                    }
                }
            }
            Key::Home => {
                self.selected = 0;
                self.scroll_top = 0;
            }
            Key::End => {
                self.selected = n - 1;
                self.scroll_top = n.saturating_sub(visible_rows.max(1));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &[&str] = &["alpha", "beta", "gamma", "delta"];

    #[test]
    fn down_moves_selection() {
        let mut lb = ListBox::new(ITEMS);
        lb.apply_key(Key::Down, 4);
        assert_eq!(lb.selected, 1);
    }

    #[test]
    fn up_clamps_at_zero() {
        let mut lb = ListBox::new(ITEMS);
        lb.apply_key(Key::Up, 4);
        assert_eq!(lb.selected, 0);
    }

    #[test]
    fn home_and_end() {
        let mut lb = ListBox::new(ITEMS);
        lb.apply_key(Key::End, 4);
        assert_eq!(lb.selected, 3);
        lb.apply_key(Key::Home, 4);
        assert_eq!(lb.selected, 0);
    }

    #[test]
    fn scroll_follows_selection() {
        let mut lb = ListBox::new(ITEMS);
        // visible_rows = 2; pressing Down 3 times should scroll
        lb.apply_key(Key::Down, 2);
        lb.apply_key(Key::Down, 2);
        lb.apply_key(Key::Down, 2);
        assert_eq!(lb.selected, 3);
        assert!(lb.scroll_top >= 2, "scroll_top should follow selection off screen");
    }

    #[test]
    fn empty_list_no_panic() {
        let mut lb = ListBox::new(&[]);
        lb.apply_key(Key::Down, 4);
        lb.apply_key(Key::Up, 4);
        assert_eq!(lb.selected, 0);
    }
}
