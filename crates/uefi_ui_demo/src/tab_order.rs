//! Full-window **Tab** order: menu → editor → optional scrollbar → each gallery control.
//! Uses [`uefi_ui::focus::cycle_tab_index`] — the demo maps indices to [`Focus`] + [`GalleryFocus`];
//! the cycling math lives in the library.

use uefi_ui::focus::cycle_tab_index;

use crate::demo_gallery::{GalleryFocus, GALLERY_FOCUS_ORDER};
use crate::layout::Focus;

/// Total slots for Tab (Shift+Tab) given whether the textarea scrollbar participates.
#[inline]
pub fn tab_slot_count(scrollbar_in_order: bool) -> usize {
    2 + (scrollbar_in_order as usize) + GALLERY_FOCUS_ORDER.len()
}

/// Linear index → focus regions. `scrollbar_in_order` must match layout (scrollbar visible & focusable).
pub fn tab_index_to_state(index: usize, scrollbar_in_order: bool) -> (Focus, GalleryFocus) {
    let n = tab_slot_count(scrollbar_in_order);
    let i = if n > 0 { index % n } else { 0 };
    if i == 0 {
        return (Focus::Menu, GalleryFocus::default());
    }
    if i == 1 {
        return (Focus::Editor, GalleryFocus::default());
    }
    let gbase = 2 + (scrollbar_in_order as usize);
    if scrollbar_in_order && i == 2 {
        return (Focus::Scrollbar, GalleryFocus::default());
    }
    let gi = i.saturating_sub(gbase);
    let gf = GALLERY_FOCUS_ORDER
        .get(gi)
        .copied()
        .unwrap_or(GalleryFocus::default());
    (Focus::Gallery, gf)
}

/// Current UI state → linear index (inverse of [`tab_index_to_state`] where possible).
pub fn tab_state_to_index(
    focus: Focus,
    gallery_focus: GalleryFocus,
    scrollbar_in_order: bool,
) -> usize {
    match focus {
        Focus::Menu => 0,
        Focus::Editor => 1,
        Focus::Scrollbar => {
            if scrollbar_in_order {
                2
            } else {
                1
            }
        }
        Focus::Gallery => {
            let gbase = 2 + (scrollbar_in_order as usize);
            let pos = GALLERY_FOCUS_ORDER
                .iter()
                .position(|&x| x == gallery_focus)
                .unwrap_or(0);
            gbase + pos
        }
    }
}

/// Apply one Tab (or Shift+Tab) step.
pub fn apply_tab(
    focus: &mut Focus,
    gallery_focus: &mut GalleryFocus,
    scrollbar_in_order: bool,
    shift: bool,
) {
    let len = tab_slot_count(scrollbar_in_order);
    let idx = tab_state_to_index(*focus, *gallery_focus, scrollbar_in_order);
    let next = cycle_tab_index(idx, len, shift);
    let (nf, ngf) = tab_index_to_state(next, scrollbar_in_order);
    *focus = nf;
    *gallery_focus = ngf;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_state_roundtrip_all_slots() {
        for sb in [false, true] {
            let n = tab_slot_count(sb);
            for i in 0..n {
                let (f, g) = tab_index_to_state(i, sb);
                let j = tab_state_to_index(f, g, sb);
                assert_eq!(j, i, "sb={sb} i={i}");
            }
        }
    }

    #[test]
    fn full_cycle_returns_to_start() {
        for sb in [false, true] {
            let n = tab_slot_count(sb);
            for start in 0..n {
                let (mut f, mut g) = tab_index_to_state(start, sb);

                for _ in 0..n {
                    apply_tab(&mut f, &mut g, sb, false);
                }
                let end = tab_state_to_index(f, g, sb);
                assert_eq!(end, start, "sb={sb} start={start}");
            }
        }
    }

    /// Randomize **starting** slot (same order as production); verify a full forward cycle restores index.
    #[test]
    fn tab_cycle_randomized_starts() {
        let mut seed: u64 = 0xDEADBEEF;
        for _ in 0..300 {
            seed = seed
                .wrapping_mul(1103515245)
                .wrapping_add(12345);
            let sb = (seed & 1) != 0;
            let n = tab_slot_count(sb);
            if n == 0 {
                continue;
            }
            let start = (seed as usize >> 8) % n;
            let (mut f, mut g) = tab_index_to_state(start, sb);
            for _ in 0..n {
                apply_tab(&mut f, &mut g, sb, false);
            }
            assert_eq!(
                tab_state_to_index(f, g, sb),
                start,
                "seed={} sb={} start={}",
                seed,
                sb,
                start
            );
        }
    }
}
