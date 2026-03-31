//! [`EFI_ABSOLUTE_POINTER_PROTOCOL`] — USB tablets / absolute mice (QEMU `-device usb-tablet`).
//! **`uefi_ui_demo`** merges with **Simple Pointer** as: apply **relative** deltas every frame, then
//! **override** with absolute **only** when that device’s **raw** `(x, y, buttons)` changes — so a
//! stale or bogus absolute device (e.g. some **VirtualBox** builds) cannot block PS/2 / relative mice.

use uefi::proto::unsafe_protocol;
use uefi::{Error, Event, Result, Status, StatusExt};
use uefi_raw::protocol::console::{AbsolutePointerMode, AbsolutePointerProtocol, AbsolutePointerState};

#[unsafe_protocol(AbsolutePointerProtocol::GUID)]
#[repr(transparent)]
pub struct AbsolutePointer(AbsolutePointerProtocol);

impl AbsolutePointer {
    pub fn reset(&mut self, extended_verification: bool) -> Result {
        unsafe { (self.0.reset)(&mut self.0, extended_verification.into()) }.to_result()
    }

    /// Current pointer position in device coordinates (see [`AbsolutePointerMode`]).
    pub fn get_state(&self) -> Result<AbsolutePointerState> {
        let mut st = AbsolutePointerState::default();
        unsafe { (self.0.get_state)(core::ptr::from_ref(&self.0), &mut st) }.to_result_with_val(|| st)
    }

    #[must_use]
    pub const fn mode(&self) -> &AbsolutePointerMode {
        unsafe { &*self.0.mode }
    }

    /// Wait event for `uefi::boot::wait_for_event` / `check_event` (needed on some OVMF builds).
    pub fn wait_for_input_event(&self) -> Result<Event> {
        unsafe { Event::from_ptr(self.0.wait_for_input) }.ok_or(Error::from(Status::UNSUPPORTED))
    }
}

/// Map absolute device coordinates to pixel coordinates `(0..w-1, 0..h-1)`.
pub fn map_abs_to_pixels(
    st: &AbsolutePointerState,
    mode: &AbsolutePointerMode,
    w: usize,
    h: usize,
) -> (i32, i32) {
    let min_x = mode.absolute_min_x;
    let max_x = mode.absolute_max_x;
    let min_y = mode.absolute_min_y;
    let max_y = mode.absolute_max_y;
    if max_x <= min_x || max_y <= min_y {
        return (0, 0);
    }
    let x = st.current_x.saturating_sub(min_x);
    let y = st.current_y.saturating_sub(min_y);
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let px = ((x * (w as u64).saturating_sub(1)) / dx) as i32;
    let py = ((y * (h as u64).saturating_sub(1)) / dy) as i32;
    (
        px.clamp(0, w.saturating_sub(1) as i32),
        py.clamp(0, h.saturating_sub(1) as i32),
    )
}

/// Any active button/contact (tablets may use non-zero bits other than bit 0).
#[inline]
pub fn primary_button(st: &AbsolutePointerState) -> bool {
    st.active_buttons != 0
}

/// Bit 1 of `active_buttons` — alternate / right-click equivalent.
#[inline]
pub fn right_button(st: &AbsolutePointerState) -> bool {
    st.active_buttons & 0x02 != 0
}

/// Stable sample for movement detection (firmwares may expose a stale absolute device + working PS/2).
#[inline]
pub fn abs_state_key(st: &AbsolutePointerState) -> (u64, u64, u32) {
    (st.current_x, st.current_y, st.active_buttons)
}
