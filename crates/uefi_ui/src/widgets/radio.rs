//! Radio group: one selected index among `count` options.

#[derive(Debug, Clone)]
pub struct RadioGroup {
    pub count: usize,
    pub selected: usize,
}

impl RadioGroup {
    pub fn new(count: usize, selected: usize) -> Self {
        let count = count.max(1);
        Self {
            count,
            selected: selected.min(count - 1),
        }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.count {
            self.selected = index;
        }
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.count;
    }

    pub fn prev(&mut self) {
        self.selected = self.selected.checked_sub(1).unwrap_or(self.count - 1);
    }
}
