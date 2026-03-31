//! Numeric field with optional increment / decrement (bind buttons in your UI).

use crate::input::{Key, KeyEvent};

#[derive(Debug, Clone)]
pub struct NumberField {
    pub value: i64,
    pub min: i64,
    pub max: i64,
    pub step: i64,
}

impl NumberField {
    pub fn new(value: i64, min: i64, max: i64, step: i64) -> Self {
        Self {
            value: value.clamp(min, max),
            min,
            max,
            step: step.max(1),
        }
    }

    pub fn increment(&mut self) {
        self.value = (self.value.saturating_add(self.step)).min(self.max);
    }

    pub fn decrement(&mut self) {
        self.value = (self.value.saturating_sub(self.step)).max(self.min);
    }

    /// Map keys: Up/Right = inc, Down/Left = dec.
    pub fn apply_key(&mut self, key: Key) -> bool {
        self.apply_key_event(&KeyEvent::new(key))
    }

    pub fn apply_key_event(&mut self, ev: &KeyEvent) -> bool {
        let key = ev.key;
        match key {
            Key::Up | Key::Right => {
                self.increment();
                true
            }
            Key::Down | Key::Left => {
                self.decrement();
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps() {
        let mut n = NumberField::new(5, 0, 10, 2);
        n.increment();
        assert_eq!(n.value, 7);
        for _ in 0..10 {
            n.increment();
        }
        assert_eq!(n.value, 10);
    }
}
