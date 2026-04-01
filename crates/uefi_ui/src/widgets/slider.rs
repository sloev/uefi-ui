//! Linear value slider.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slider {
    pub min: f32,
    pub max: f32,
    pub value: f32,
}

impl Slider {
    pub fn new(min: f32, max: f32, value: f32) -> Self {
        let mut s = Self { min, max, value };
        s.clamp_value();
        s
    }

    fn clamp_value(&mut self) {
        let lo = self.min.min(self.max);
        let hi = self.max.max(self.min);
        self.min = lo;
        self.max = hi;
        self.value = self.value.clamp(lo, hi);
    }

    /// Set from pointer position: `t` in 0..=1 along the track.
    pub fn set_from_ratio(&mut self, t: f32) {
        let t = t.clamp(0.0, 1.0);
        self.value = self.min + t * (self.max - self.min);
    }

    pub fn ratio(&self) -> f32 {
        let d = self.max - self.min;
        if d.abs() < f32::EPSILON {
            return 0.0;
        }
        (self.value - self.min) / d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_midpoint() {
        let s = Slider::new(0.0, 100.0, 50.0);
        assert!((s.ratio() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn set_from_ratio_roundtrip() {
        let mut s = Slider::new(0.0, 200.0, 0.0);
        s.set_from_ratio(0.75);
        assert!((s.value - 150.0).abs() < 1e-4);
    }

    #[test]
    fn value_clamped_on_new() {
        let s = Slider::new(10.0, 20.0, 99.0);
        assert_eq!(s.value, 20.0);
    }

    #[test]
    fn zero_range_ratio_is_zero() {
        let s = Slider::new(5.0, 5.0, 5.0);
        assert_eq!(s.ratio(), 0.0);
    }
}
