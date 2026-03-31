//! Top menubar plus optional per-item submenu (keyboard navigation).
//!
//! With a submenu open: **Up/Down** move the highlight vertically; **Left/Right** switch the
//! top-level menu and keep a dropdown open when the new top item has items; **Enter** activates;
//! **Escape** closes the dropdown.

use crate::input::{Key, KeyEvent};
use crate::menu::MenuBar;

/// Top-level horizontal bar + parallel submenu label lists (`None` = no submenu).
#[derive(Debug, Clone)]
pub struct NavBar<'a> {
    pub top: MenuBar<'a>,
    /// Same length as `top` labels: `Some(&[...])` gives submenu entries.
    pub submenus: &'a [Option<&'a [&'a str]>],
    /// `(top_index, sub_index)` when a submenu is visible.
    pub open: Option<(usize, usize)>,
}

impl<'a> NavBar<'a> {
    pub fn new(top_labels: &'a [&'a str], submenus: &'a [Option<&'a [&'a str]>]) -> Self {
        Self {
            top: MenuBar::new(top_labels),
            submenus,
            open: None,
        }
    }

    pub fn close_submenu(&mut self) {
        self.open = None;
    }

    /// Pointer picked top-level item `i`: if that item’s submenu is already open, close it;
    /// otherwise focus `i` and open its submenu when it has entries.
    pub fn pointer_activate_top(&mut self, i: usize) {
        self.top.set_focused_index(i);
        match self.submenus.get(i) {
            Some(Some(subs)) if !subs.is_empty() => {
                if let Some((ti, _)) = self.open {
                    if ti == i {
                        self.open = None;
                        return;
                    }
                }
                self.open = Some((i, 0));
            }
            _ => {
                self.open = None;
            }
        }
    }

    /// Re-open dropdown for the current top focus after switching menus (if that item has a list).
    fn sync_open_to_top(&mut self) {
        let ti = self.top.focused_index();
        self.open = match self.submenus.get(ti) {
            Some(Some(subs)) if !subs.is_empty() => Some((ti, 0)),
            _ => None,
        };
    }

    /// Down opens or moves down in the submenu; Up moves up or closes at the top row.
    pub fn apply_key(&mut self, key: Key) -> Option<(usize, Option<usize>)> {
        self.apply_key_event(&KeyEvent::new(key))
    }

    /// Returns `Some((top_index, sub_index))` when the user confirms a submenu row with **Enter**,
    /// or `Some((top_index, None))` for **Enter** on the bar with no open dropdown.
    pub fn apply_key_event(&mut self, ev: &KeyEvent) -> Option<(usize, Option<usize>)> {
        let key = ev.key;
        match key {
            Key::Down => {
                if let Some((ti, si)) = self.open {
                    if let Some(Some(subs)) = self.submenus.get(ti) {
                        if !subs.is_empty() && si + 1 < subs.len() {
                            self.open = Some((ti, si + 1));
                        }
                    }
                } else {
                    let ti = self.top.focused_index();
                    if let Some(Some(subs)) = self.submenus.get(ti) {
                        if !subs.is_empty() {
                            self.open = Some((ti, 0));
                        }
                    }
                }
                None
            }
            Key::Up => {
                if let Some((ti, si)) = self.open {
                    if si == 0 {
                        self.open = None;
                    } else {
                        self.open = Some((ti, si - 1));
                    }
                }
                None
            }
            Key::Left | Key::Right => {
                if self.open.is_some() {
                    match key {
                        Key::Left => self.top.nav_prev(),
                        Key::Right => self.top.nav_next(),
                        _ => {}
                    }
                    self.sync_open_to_top();
                } else {
                    match key {
                        Key::Left => self.top.nav_prev(),
                        Key::Right => self.top.nav_next(),
                        _ => {}
                    }
                }
                None
            }
            Key::Enter => {
                if let Some((ti, si)) = self.open {
                    return Some((ti, Some(si)));
                }
                Some((self.top.focused_index(), None))
            }
            Key::Escape => {
                self.open = None;
                None
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static FILE: &[&str] = &["A", "B"];
    static SUBS: &[Option<&[&str]>] = &[Some(FILE), None];

    #[test]
    fn down_opens_then_moves() {
        let labels: &[&str] = &["File", "Help"];
        let mut n = NavBar::new(labels, SUBS);
        assert!(n.apply_key_event(&KeyEvent::new(Key::Down)).is_none());
        assert_eq!(n.open, Some((0, 0)));
        n.apply_key_event(&KeyEvent::new(Key::Down));
        assert_eq!(n.open, Some((0, 1)));
    }

    #[test]
    fn left_switches_top_while_open() {
        let labels: &[&str] = &["File", "Help"];
        let mut n = NavBar::new(labels, SUBS);
        n.apply_key_event(&KeyEvent::new(Key::Down));
        assert_eq!(n.open, Some((0, 0)));
        n.apply_key_event(&KeyEvent::new(Key::Right));
        assert_eq!(n.top.focused_index(), 1);
        assert!(n.open.is_none());
    }

    #[test]
    fn pointer_toggle_closes_same_top_submenu() {
        let labels: &[&str] = &["File", "Help"];
        let mut n = NavBar::new(labels, SUBS);
        n.pointer_activate_top(0);
        assert_eq!(n.open, Some((0, 0)));
        n.pointer_activate_top(0);
        assert!(n.open.is_none());
    }
}
