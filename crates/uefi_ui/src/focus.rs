//! **Tab / roving focus** helpers: cycle a linear index without embedding app-specific enums.
//!
//! Applications build a list of focusable regions (menu, editor, each widget, …), map the active
//! index to their own state each frame, and call [`cycle_tab_index`] on **Tab** / **Shift+Tab**.
//! This mirrors the idea of a flat tab order (cf. roving `tabIndex` in web toolkits).

/// Advance `current` in `0..len` (wrapping). If `len == 0`, returns `0`.
#[inline]
pub fn cycle_tab_index(current: usize, len: usize, reverse: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if reverse {
        (current + len - 1) % len
    } else {
        (current + 1) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_roundtrip_forward() {
        for len in 1..=64 {
            for start in 0..len {
                let mut i = start;
                for _ in 0..len {
                    i = cycle_tab_index(i, len, false);
                }
                assert_eq!(i, start, "len={len} start={start}");
            }
        }
    }

    #[test]
    fn cycle_roundtrip_backward() {
        for len in 1..=64 {
            for start in 0..len {
                let mut i = start;
                for _ in 0..len {
                    i = cycle_tab_index(i, len, true);
                }
                assert_eq!(i, start, "len={len} start={start}");
            }
        }
    }

    /// Pseudo-random lengths and starts (no `rand` dependency): LCG on a seed.
    #[test]
    fn cycle_randomized_lengths() {
        let mut seed: u64 = 0xC0FFEEu64;
        for _ in 0..500 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let len = (seed % 48) as usize + 1;
            let start = ((seed >> 16) as usize) % len;

            let mut i = start;
            for _ in 0..len {
                i = cycle_tab_index(i, len, false);
            }
            assert_eq!(i, start, "forward seed={seed} len={len} start={start}");

            let mut j = start;
            for _ in 0..len {
                j = cycle_tab_index(j, len, true);
            }
            assert_eq!(j, start, "reverse seed={seed} len={len} start={start}");
        }
    }

    #[test]
    fn len_zero_no_panic() {
        assert_eq!(cycle_tab_index(3, 0, false), 0);
        assert_eq!(cycle_tab_index(3, 0, true), 0);
    }
}
