//! Simple line / sparkline graph samples for plotting.

use alloc::vec::Vec;

use embedded_graphics::geometry::Point;
use embedded_graphics::primitives::Rectangle;

/// Ring buffer of samples; older values drop when over capacity.
#[derive(Debug, Clone, PartialEq)]
pub struct LineGraph {
    pub samples: Vec<f32>,
    pub cap: usize,
}

impl LineGraph {
    pub fn new(cap: usize) -> Self {
        Self {
            samples: Vec::new(),
            cap: cap.max(2),
        }
    }

    pub fn push(&mut self, v: f32) {
        if self.samples.len() >= self.cap {
            self.samples.remove(0);
        }
        self.samples.push(v);
    }

    /// Points in pixel space for a polyline inside `bounds` (min/max auto from samples).
    pub fn points(&self, bounds: Rectangle) -> Vec<Point> {
        let n = self.samples.len();
        if n < 2 {
            return Vec::new();
        }
        let (min_v, max_v) = min_max(&self.samples);
        let span = (max_v - min_v).max(1e-6);
        let x0 = bounds.top_left.x;
        let y0 = bounds.top_left.y;
        let w = bounds.size.width as i32;
        let h = bounds.size.height.max(1) as i32;
        let mut pts = Vec::with_capacity(n);
        for (i, &v) in self.samples.iter().enumerate() {
            let t = i as f32 / (n - 1).max(1) as f32;
            let nx = x0 + (t * w as f32 + 0.5) as i32;
            let ny = y0 + h - (((v - min_v) / span) * h as f32 + 0.5) as i32;
            pts.push(Point::new(nx, ny));
        }
        pts
    }
}

fn min_max(s: &[f32]) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for &v in s {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !lo.is_finite() {
        lo = 0.0;
    }
    if !hi.is_finite() {
        hi = 1.0;
    }
    (lo, hi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_graphics::prelude::{Point, Size};

    #[test]
    fn points_non_empty() {
        let mut g = LineGraph::new(8);
        for i in 0..8 {
            g.push(i as f32);
        }
        let r = Rectangle::new(Point::zero(), Size::new(100, 50));
        assert_eq!(g.points(r).len(), 8);
    }
}
