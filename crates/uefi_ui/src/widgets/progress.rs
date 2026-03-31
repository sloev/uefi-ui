//! Progress bar (0..=1).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressBar {
    /// Inclusive 0..=1.
    pub value: f32,
}

impl ProgressBar {
    pub fn new(value: f32) -> Self {
        Self {
            value: value.clamp(0.0, 1.0),
        }
    }

    pub fn set(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }
}
