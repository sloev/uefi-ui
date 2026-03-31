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
