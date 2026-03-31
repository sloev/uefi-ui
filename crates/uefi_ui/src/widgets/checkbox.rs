//! Checkbox (boolean, distinct from toggle semantics in apps).

use crate::widgets::Toggle;

/// Two-state checkbox; maps to the same data as [`Toggle`] but named for forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkbox {
    pub inner: Toggle,
}

impl Checkbox {
    pub const fn new(checked: bool) -> Self {
        Self {
            inner: Toggle::new(checked),
        }
    }

    pub fn checked(&self) -> bool {
        self.inner.on
    }

    pub fn toggle(&mut self) {
        self.inner.flip();
    }

    pub fn set(&mut self, checked: bool) {
        self.inner.set(checked);
    }
}
