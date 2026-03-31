//! Shared **UEFI demo UI** (layout + painting + widget gallery). The firmware binary links this
//! library; the host **`uefi_ui_prototype`** can render the same frame into a [`BgrxFramebuffer`]
//! buffer for PNG/SDL (see [`scene::paint_demo_snapshot`] in this crate).

#![no_std]

extern crate alloc;

pub mod demo_gallery;
pub mod layout;
pub mod scene;
pub mod tab_order;
pub mod ttf_text;
