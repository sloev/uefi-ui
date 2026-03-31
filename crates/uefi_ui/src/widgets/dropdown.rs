//! Single-select dropdown (options are borrowed for the frame).

use crate::input::{Key, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownAction {
    /// Selection changed to this index
    Selected(usize),
    /// List closed without change
    Closed,
}

#[derive(Debug, Clone)]
pub struct Dropdown<'a> {
    pub options: &'a [&'a str],
    pub selected: usize,
    pub open: bool,
    focus: usize,
}

impl<'a> Dropdown<'a> {
    pub fn new(options: &'a [&'a str], selected: usize) -> Self {
        let n = options.len();
        let sel = if n == 0 { 0 } else { selected.min(n - 1) };
        Self {
            options,
            selected: sel,
            open: false,
            focus: sel,
        }
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
        if self.open {
            self.focus = self.selected;
        }
    }

    /// Highlighted row index while the list is open (keyboard / pending selection).
    pub fn menu_focus_index(&self) -> usize {
        self.focus
    }

    /// While open, set the highlighted row (e.g. pointer over list).
    pub fn set_menu_focus_index(&mut self, index: usize) {
        if index < self.options.len() {
            self.focus = index;
        }
    }

    pub fn apply_key(&mut self, key: Key) -> Option<DropdownAction> {
        self.apply_key_event(&KeyEvent::new(key))
    }

    pub fn apply_key_event(&mut self, ev: &KeyEvent) -> Option<DropdownAction> {
        let key = ev.key;
        if !self.open {
            match key {
                Key::Enter | Key::Down => {
                    self.open = true;
                    self.focus = self.selected;
                    None
                }
                Key::Up | Key::Left | Key::Right => None,
                Key::Escape => None,
                _ => None,
            }
        } else {
            match key {
                Key::Up => {
                    if self.focus > 0 {
                        self.focus -= 1;
                    }
                    None
                }
                Key::Down => {
                    if self.focus + 1 < self.options.len() {
                        self.focus += 1;
                    }
                    None
                }
                Key::Enter => {
                    self.selected = self.focus;
                    self.open = false;
                    Some(DropdownAction::Selected(self.selected))
                }
                Key::Escape => {
                    self.open = false;
                    Some(DropdownAction::Closed)
                }
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_on_enter() {
        let opts: &[&str] = &["a", "b", "c"];
        let mut d = Dropdown::new(opts, 0);
        d.toggle_open();
        d.apply_key(Key::Down);
        d.apply_key(Key::Down);
        assert_eq!(d.apply_key(Key::Enter), Some(DropdownAction::Selected(2)));
        assert!(!d.open);
    }
}
