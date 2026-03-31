//! Virtual keys and small keyboard→widget adapters.
//!
//! **S4 policy:** [`apply_key_event_to_menu`] uses only [`KeyEvent::key`]; [`Modifiers`] are ignored
//! for the horizontal menubar. Widgets that care (e.g. [`crate::widgets::TextArea`]) read
//! modifiers from [`KeyEvent`]. Navigation keys repeat as delivered by the firmware; **Enter**
//! activation is debounced via [`KeyboardInput`] so one physical press yields one activation until
//! release ([`key_event_up`]).

use crate::menu::MenuBar;

/// Logical keys for widgets (map from UEFI `SimpleTextInputEx` / PS/2 in your app).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    /// Unicode from firmware text input
    Character(char),
    /// Opaque scan / vendor code (e.g. unmapped media keys).
    Other(u16),
    /// Function key: `n` in 1..=12 for F1..=F12.
    Function(u8),
}

/// Shift / Ctrl / Alt as reported by the firmware (or synthesized by the app).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
    };
}

/// One physical key event: navigation keys plus optional modifiers (for text selection, shortcuts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyEvent {
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            modifiers: Modifiers::NONE,
        }
    }

    pub const fn with_modifiers(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
}

/// S4.2: Debounces **Enter** for [`MenuBar`] activation — one activation per physical press
/// (use with [`apply_key_event_to_menu`] / [`key_event_up`]).
///
/// The specification calls this behavior “`KeyboardInput::step`”; the implementation is
/// [`enter_edge_on_key_down`] (historical name: [`KeyRepeatGate::on_enter_edge`]).
#[derive(Debug, Default)]
pub struct KeyboardInput {
    enter_armed: bool,
}

/// Deprecated alias for [`KeyboardInput`] (same type).
pub type KeyRepeatGate = KeyboardInput;

impl KeyboardInput {
    /// `true` when this key-down should count as a fresh **Enter** (edge), `false` on auto-repeat.
    pub fn enter_edge_on_key_down(&mut self, down: bool) -> bool {
        if down {
            if self.enter_armed {
                false
            } else {
                self.enter_armed = true;
                true
            }
        } else {
            self.enter_armed = false;
            false
        }
    }

    /// Same as [`Self::enter_edge_on_key_down`] (kept for older call sites).
    pub fn on_enter_edge(&mut self, down: bool) -> bool {
        self.enter_edge_on_key_down(down)
    }
}

/// Apply one key to a top-level [`MenuBar`]. Returns `Some(index)` when Enter activates.
pub fn apply_key_to_menu(bar: &mut MenuBar<'_>, key: Key, gate: &mut KeyboardInput) -> Option<usize> {
    apply_key_event_to_menu(bar, &KeyEvent::new(key), gate)
}

/// Same as [`apply_key_to_menu`], but uses [`KeyEvent::key`] only (modifiers ignored).
pub fn apply_key_event_to_menu(
    bar: &mut MenuBar<'_>,
    ev: &KeyEvent,
    gate: &mut KeyboardInput,
) -> Option<usize> {
    match ev.key {
        Key::Left => {
            bar.nav_prev();
            None
        }
        Key::Right => {
            bar.nav_next();
            None
        }
        Key::Enter => {
            if gate.on_enter_edge(true) {
                bar.activate_focused()
            } else {
                None
            }
        }
        Key::Escape
        | Key::Up
        | Key::Down
        | Key::Tab
        | Key::Backspace
        | Key::Delete
        | Key::Home
        | Key::End
        | Key::PageUp
        | Key::PageDown
        | Key::Character(_)
        | Key::Other(_)
        | Key::Function(_) => None,
    }
}

/// Call when Enter is released (multi-frame polling).
pub fn key_up(_bar: &mut MenuBar<'_>, key: Key, gate: &mut KeyboardInput) {
    if matches!(key, Key::Enter) {
        gate.on_enter_edge(false);
    }
}

pub fn key_event_up(_bar: &mut MenuBar<'_>, ev: &KeyEvent, gate: &mut KeyboardInput) {
    key_up(_bar, ev.key, gate);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_once_until_release() {
        let labels: &[&str] = &["A", "B"];
        let mut bar = MenuBar::new(labels);
        let mut gate = KeyboardInput::default();
        assert_eq!(
            apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Enter), &mut gate),
            Some(0)
        );
        assert_eq!(
            apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Enter), &mut gate),
            None
        );
        key_event_up(&mut bar, &KeyEvent::new(Key::Enter), &mut gate);
        assert_eq!(
            apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Enter), &mut gate),
            Some(0)
        );
    }

    #[test]
    fn arrows_move_focus() {
        let labels: &[&str] = &["A", "B", "C"];
        let mut bar = MenuBar::new(labels);
        let mut gate = KeyboardInput::default();
        apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Right), &mut gate);
        apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Right), &mut gate);
        assert_eq!(bar.focused_index(), 2);
        apply_key_event_to_menu(&mut bar, &KeyEvent::new(Key::Left), &mut gate);
        assert_eq!(bar.focused_index(), 1);
    }
}
