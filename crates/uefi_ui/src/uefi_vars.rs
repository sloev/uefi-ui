//! Load / store [`crate::settings::SettingsBlob`] in **UEFI NVRAM** (`SetVariable` / `GetVariable`).
//!
//! Pick a unique vendor GUID per product. Names must be valid UEFI variable names (see firmware limits).

use uefi::runtime::{self, VariableAttributes, VariableVendor};
use uefi::{guid, Result};

/// Namespace for `uefi_ui` settings — **replace** with your own GUID for shipping firmware.
pub const SETTINGS_VENDOR: VariableVendor =
    VariableVendor(guid!("a7b8c9d0-e1f2-4a3b-8c4d-5e6f708192a0"));

fn nv_attributes() -> VariableAttributes {
    VariableAttributes::NON_VOLATILE
        | VariableAttributes::BOOTSERVICE_ACCESS
        | VariableAttributes::RUNTIME_ACCESS
}

/// Read raw bytes (empty / missing variable returns `Ok(None)`).
pub fn get_settings_raw(name: &uefi::CStr16) -> Result<Option<alloc::boxed::Box<[u8]>>> {
    match runtime::get_variable_boxed(name, &SETTINGS_VENDOR) {
        Ok((b, _)) => Ok(Some(b)),
        Err(e) if e.status() == uefi::Status::NOT_FOUND => Ok(None),
        Err(e) => Err(e),
    }
}

/// Persist raw bytes (replaces existing value).
pub fn set_settings_raw(name: &uefi::CStr16, data: &[u8]) -> Result<()> {
    runtime::set_variable(name, &SETTINGS_VENDOR, nv_attributes(), data)
}

/// Encode and store a [`crate::settings::SettingsBlob`].
pub fn save_settings_blob(name: &uefi::CStr16, blob: &crate::settings::SettingsBlob) -> Result<()> {
    let v = blob.encode();
    set_settings_raw(name, &v)
}

/// Load and decode; corrupt or missing data returns `Ok(None)`.
pub fn load_settings_blob(name: &uefi::CStr16) -> Result<Option<crate::settings::SettingsBlob>> {
    let Some(raw) = get_settings_raw(name)? else {
        return Ok(None);
    };
    Ok(crate::settings::SettingsBlob::decode(&raw))
}
