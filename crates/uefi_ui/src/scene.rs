//! Immediate-mode frame state (egui-style ids + per-frame scratch).
//!
//! **S2 focus:** [`UiFrame::focus`] is an optional opaque [`WidgetId`]. The library does not map
//! ids to widgets; **applications** decide which region owns focus (menubar vs editor vs gallery,
//! etc.). A higher-level registry could be built on top by storing `WidgetId → handler` maps in
//! app code if needed.

use alloc::vec::Vec;

/// Stable widget / region id (hash or counter assigned by app).
pub type WidgetId = u64;

/// Per-frame UI state: focus, pointer, and actions collected this frame.
#[derive(Debug)]
pub struct UiFrame {
    pub focus: Option<WidgetId>,
    pub pointer_x: i32,
    pub pointer_y: i32,
    pub pointer_left: bool,
    activated: Vec<WidgetId>,
}

impl Default for UiFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl UiFrame {
    pub fn new() -> Self {
        Self {
            focus: None,
            pointer_x: 0,
            pointer_y: 0,
            pointer_left: false,
            activated: Vec::new(),
        }
    }

    /// Call at the start of each frame (keeps focus; clears transient actions).
    pub fn begin_frame(&mut self) {
        self.activated.clear();
    }

    /// Call at end of frame after processing input.
    pub fn end_frame(&mut self) {}

    pub fn set_focus(&mut self, id: WidgetId) {
        self.focus = Some(id);
    }

    pub fn queue_activate(&mut self, id: WidgetId) {
        self.activated.push(id);
    }

    pub fn take_activations(&mut self) -> impl Iterator<Item = WidgetId> + '_ {
        self.activated.drain(..)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s2_begin_frame_clears_activations() {
        let mut ui = UiFrame::new();
        ui.queue_activate(1);
        ui.begin_frame();
        assert_eq!(ui.take_activations().count(), 0);
    }
}
