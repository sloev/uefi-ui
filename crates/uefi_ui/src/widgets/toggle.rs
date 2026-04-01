//! Boolean toggle.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggle {
    pub on: bool,
}

impl Toggle {
    pub const fn new(on: bool) -> Self {
        Self { on }
    }

    pub fn flip(&mut self) {
        self.on = !self.on;
    }

    pub fn set(&mut self, on: bool) {
        self.on = on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_inverts() {
        let mut t = Toggle::new(false);
        t.flip();
        assert!(t.on);
        t.flip();
        assert!(!t.on);
    }

    #[test]
    fn set_overrides() {
        let mut t = Toggle::new(true);
        t.set(false);
        assert!(!t.on);
    }
}
