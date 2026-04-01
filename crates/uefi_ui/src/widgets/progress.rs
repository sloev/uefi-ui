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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_clamped_to_zero_one() {
        let p = ProgressBar::new(2.5);
        assert_eq!(p.value, 1.0);
        let p2 = ProgressBar::new(-1.0);
        assert_eq!(p2.value, 0.0);
    }

    #[test]
    fn set_clamps() {
        let mut p = ProgressBar::new(0.5);
        p.set(1.5);
        assert_eq!(p.value, 1.0);
    }
}
