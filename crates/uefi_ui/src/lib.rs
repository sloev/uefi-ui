//! Immediate-mode UI helpers for **UEFI** and for **host unit tests**.
//!
//! # What UEFI Rust can and cannot do
//!
//! The `x86_64-unknown-uefi` target is **`#![no_std]`**: there is **no `std`**
//! (no OS threads, processes, filesystem as you know it, TCP stack, etc.). You get:
//!
//! - [`core`] — always.
//! - [`alloc`] — **only if** the firmware app installs a **global allocator** (see
//!   [`uefi::allocator`] in the `uefi` crate). Without it, types like [`alloc::vec::Vec`]
//! and font raster buffers will not link.
//!
//! What you *do* have are **UEFI boot services** (until you call `ExitBootServices`) and
//! **protocols**, exposed safely by [`uefi`]: GOP framebuffer, Simple Text Input, Simple
//! Pointer, block I/O, Simple File System on removable media, etc.
//!
//! This crate is written so that **all library code** uses only `core` + `alloc` (no `std`).
//! **`cargo test` runs on the host** and enables `std` for the test harness only; that does
//! not change what ships on UEFI.
//!
//! Enable the **`uefi`** feature for glue that talks to [`uefi`] protocols (NVRAM, FAT listing).
//!
//! **Theme** ([`theme::Theme`]), **layout** ([`layout`]), **popovers** ([`popover`]), and **widgets**
//! ([`widgets`]) are portable. **Bedrock chrome** is optional: [`bedrock::BedrockBevel`] and
//! [`bedrock_controls`] help draw classic 3D controls; apps may ignore them and build any other look.
//!
//! [`uefi::allocator`]: https://docs.rs/uefi/latest/uefi/allocator/index.html

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod bedrock;
/// Optional Bedrock-style **drawing** helpers (checkbox, radio, slider chrome). Widgets stay state-only.
pub mod bedrock_controls;
pub mod editor_settings;
pub mod file_picker;
pub mod focus;
pub mod font;
pub mod framebuffer;
pub mod input;
pub mod layout;
pub mod menu;
pub mod png;
pub mod pointer;
pub mod popover;
pub mod scene;
pub mod settings;
pub mod theme;
pub mod tree_view;
pub mod widgets;
pub mod window;

#[cfg(feature = "uefi")]
pub mod uefi_fs;
#[cfg(feature = "uefi")]
pub use uefi_fs::{
    find_user_fs_handles, list_simple_fs_handles, open_directory_at_path, SimpleFsIo,
};
#[cfg(feature = "uefi")]
pub mod uefi_vars;

pub use embedded_graphics;
