//! Keyboard layout management via **HII Database Protocol**.
//!
//! Provides functions to list, load, and set keyboard layouts in UEFI firmware.
//! Layouts are persisted to **NVRAM** across reboots.
//!
//! # Usage
//!
//! ```ignore
//! use uefi_ui::keyboard_layout::{list_keyboard_layouts, set_active_keyboard_layout, load_active_keyboard_layout};
//!
//! // List available layouts
//! let layouts = list_keyboard_layouts().unwrap_or_default();
//!
//! // Set Danish keyboard by name
//! if let Some(danish) = layouts.iter().find(|l| l.descriptor == "Danish") {
//!     set_active_keyboard_layout(&danish.guid).ok();
//! }
//!
//! // Or by searching
//! if let Some(danish) = layouts.iter().find(|l| l.descriptor.contains("Danish")) {
//!     set_active_keyboard_layout(&danish.guid).ok();
//! }
//!
//! // Load saved layout on boot
//! if let Ok(Some(guid)) = load_active_keyboard_layout() {
//!     set_active_keyboard_layout(&guid).ok();
//! }
//! ```
//!
//! # Implementation Notes
//!
//! Uses the **HII Database Protocol** (`EFI_HII_DATABASE_PROTOCOL`) functions:
//! - `find_keyboard_layouts` — enumerate available keyboard layout GUIDs
//! - `get_keyboard_layout` — retrieve layout descriptor (contains human-readable name)
//! - `set_keyboard_layout` — activate a specific layout
//!
//! The active layout GUID is persisted to UEFI NVRAM using a vendor-specific variable.

#![cfg(feature = "uefi")]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use uefi::boot::{get_handle_for_protocol, open_protocol_exclusive};
use uefi::proto::hii::database::HiiDatabase;
use uefi::Result;
use uefi_raw::protocol::hii::database::HiiDatabaseProtocol;
use uefi_raw::Guid;

use crate::uefi_vars::{get_settings_raw, set_settings_raw, SETTINGS_VENDOR};

/// NVRAM variable name for storing the active keyboard layout GUID.
pub const KBD_LAYOUT_NVRAM_NAME: &str = "KbdLayout";

/// A keyboard layout descriptor.
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    /// Unique identifier for this layout.
    pub guid: Guid,
    /// Human-readable descriptor string (e.g., "Danish", "US English").
    pub descriptor: String,
}

/// Known keyboard layout GUIDs mapped to their human-readable names.
/// These are GUIDs commonly used by different firmware vendors.
///
/// Source: Various firmware implementations (OVMF, EDK2, etc.)
pub fn get_known_layout_name(_guid: &Guid) -> Option<&'static str> {
    // Common keyboard layout GUIDs from various firmware implementations
    // These are typically found in HII packages
    
    // Check against known layout GUIDs
    // Note: These are example GUIDs - actual GUIDs depend on the firmware
    // In practice, we query the firmware for layout descriptors
    
    // For now, we rely on the runtime descriptor query
    // This function is a fallback for when descriptor parsing fails
    None
}

/// A GUID that uniquely identifies a keyboard layout.
pub type KeyboardLayoutGuid = Guid;

/// Which interactive zone of the keyboard layout dialog currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardLayoutPickerFocus {
    /// The list of keyboard layouts.
    List,
    /// The OK button.
    OkButton,
    /// The Cancel button.
    CancelButton,
}

impl KeyboardLayoutPickerFocus {
    /// All focusable zones in Tab order.
    const ORDER: [KeyboardLayoutPickerFocus; 3] = [
        KeyboardLayoutPickerFocus::List,
        KeyboardLayoutPickerFocus::OkButton,
        KeyboardLayoutPickerFocus::CancelButton,
    ];

    /// Advance focus (Tab).
    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|&z| z == self).unwrap_or(0);
        let next_idx = (idx + 1) % Self::ORDER.len();
        Self::ORDER[next_idx]
    }

    /// Retreat focus (Shift+Tab).
    pub fn prev(self) -> Self {
        let idx = Self::ORDER.iter().position(|&z| z == self).unwrap_or(0);
        let prev_idx = (idx + Self::ORDER.len() - 1) % Self::ORDER.len();
        Self::ORDER[prev_idx]
    }
}

/// State for the keyboard layout picker dialog.
#[derive(Debug, Clone)]
pub struct KeyboardLayoutPickerState {
    /// Available keyboard layouts.
    pub layouts: Vec<KeyboardLayout>,
    /// Currently selected layout index.
    pub selected: usize,
    /// Scroll position for the layout list.
    pub scroll_top: usize,
    /// Which zone has keyboard focus.
    pub focus: KeyboardLayoutPickerFocus,
}

impl KeyboardLayoutPickerState {
    /// Create a new picker state with the given layouts.
    pub fn new(layouts: Vec<KeyboardLayout>) -> Self {
        Self {
            layouts,
            selected: 0,
            scroll_top: 0,
            focus: KeyboardLayoutPickerFocus::List,
        }
    }

    /// Reload layouts from firmware.
    pub fn reload(&mut self) -> Result<()> {
        self.layouts = list_keyboard_layouts()?;
        self.selected = 0;
        self.scroll_top = 0;
        Ok(())
    }

    /// Get the currently selected layout, if any.
    pub fn selected_layout(&self) -> Option<&KeyboardLayout> {
        self.layouts.get(self.selected)
    }

    /// Returns true if the list has focus.
    pub fn list_focused(&self) -> bool {
        matches!(self.focus, KeyboardLayoutPickerFocus::List)
    }

    /// Returns true if OK button has focus.
    pub fn ok_focused(&self) -> bool {
        matches!(self.focus, KeyboardLayoutPickerFocus::OkButton)
    }

    /// Returns true if Cancel button has focus.
    pub fn cancel_focused(&self) -> bool {
        matches!(self.focus, KeyboardLayoutPickerFocus::CancelButton)
    }

    /// Number of available layouts.
    pub fn len(&self) -> usize {
        self.layouts.len()
    }

    /// Check if there are no layouts available.
    pub fn is_empty(&self) -> bool {
        self.layouts.is_empty()
    }
}

impl Default for KeyboardLayoutPickerState {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// Read a UTF-16LE null-terminated string from a byte slice.
/// Returns the string content and the number of bytes consumed (including null).
fn read_utf16le_string(data: &[u8]) -> (String, usize) {
    let mut chars = Vec::new();
    let mut i = 0;
    
    while i + 1 < data.len() {
        let lo = data[i] as u16;
        let hi = data[i + 1] as u16;
        let code = (hi << 8) | lo;
        
        if code == 0 {
            // Null terminator
            i += 2;
            break;
        }
        
        if let Some(c) = core::char::from_u32(code as u32) {
            if c.is_ascii() || !c.is_control() {
                chars.push(c);
            }
        }
        i += 2;
    }
    
    (chars.into_iter().collect(), i)
}

/// List all available keyboard layouts from the firmware.
///
/// Returns an empty vector if:
/// - The HII Database Protocol is not available
/// - The firmware reports no keyboard layouts
/// - Any error occurs during enumeration
///
/// Each layout includes both its GUID and a human-readable descriptor string
/// that is extracted from the firmware's HII database.
pub fn list_keyboard_layouts() -> Result<Vec<KeyboardLayout>> {
    // Get the HII Database protocol handle
    let handle = match get_handle_for_protocol::<HiiDatabase>() {
        Ok(h) => h,
        Err(_) => return Ok(Vec::new()),
    };

    // Open the protocol exclusively
    // This returns ScopedProtocol which implements DerefMut to &mut HiiDatabase
    let mut hii_db = match open_protocol_exclusive::<HiiDatabase>(handle) {
        Ok(db) => db,
        Err(_) => return Ok(Vec::new()),
    };

    // Access the underlying raw protocol through the wrapper
    // HiiDatabase is #[repr(transparent)] so we can access the inner HiiDatabaseProtocol
    // SAFETY: This is safe because HiiDatabase is a transparent wrapper around HiiDatabaseProtocol
    let hii_raw: &mut HiiDatabaseProtocol = unsafe {
        // Dereference ScopedProtocol to get &mut HiiDatabase, then cast to &mut HiiDatabaseProtocol
        &mut *(&mut *hii_db as *mut HiiDatabase as *mut HiiDatabaseProtocol)
    };

    // First, get the number of available layouts
    let mut layout_count: u16 = 0;
    let status = unsafe {
        (hii_raw.find_keyboard_layouts)(
            hii_raw,
            &mut layout_count,
            core::ptr::null_mut(),
        )
    };

    // If no layouts or error, return empty
    if status.is_error() && status != uefi::Status::BUFFER_TOO_SMALL {
        return Ok(Vec::new());
    }

    if layout_count == 0 {
        return Ok(Vec::new());
    }

    // Allocate buffer for GUIDs
    let mut guids: Vec<Guid> = Vec::with_capacity(layout_count as usize);
    let status = unsafe {
        (hii_raw.find_keyboard_layouts)(
            hii_raw,
            &mut layout_count,
            guids.as_mut_ptr() as *mut _,
        )
    };

    if status.is_error() {
        return Ok(Vec::new());
    }

    // SAFETY: We allocated enough space for layout_count GUIDs
    unsafe {
        guids.set_len(layout_count as usize);
    }

    // For each GUID, get the full keyboard layout and extract the descriptor string
    let mut layouts = Vec::with_capacity(guids.len());
    
    for guid in &guids {
        // Try to get the descriptor string from the keyboard layout
        // Use the mutable raw protocol reference
        let descriptor = get_layout_descriptor_string(hii_raw, guid);
        
        let descriptor_str = match descriptor {
            Some(desc) if !desc.is_empty() => desc,
            _ => {
                // Fallback: create a descriptive name from GUID
                let bytes = guid.to_bytes();
                // Format as "Layout: first 4 bytes as hex"
                format!("Layout {:#010x}", u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            }
        };
        
        layouts.push(KeyboardLayout {
            guid: *guid,
            descriptor: descriptor_str,
        });
    }

    Ok(layouts)
}

/// Retrieve the descriptor string from a keyboard layout by calling get_keyboard_layout.
fn get_layout_descriptor_string(
    hii_raw: &mut HiiDatabaseProtocol,
    guid: &Guid,
) -> Option<String> {
    // First, get the required buffer size
    let mut layout_length: u16 = 0;
    let status = unsafe {
        (hii_raw.get_keyboard_layout)(
            hii_raw,
            guid as *const _ as *const _,
            &mut layout_length,
            core::ptr::null_mut(),
        )
    };

    if status == uefi::Status::NOT_FOUND {
        return None;
    }

    if status.is_error() && status != uefi::Status::BUFFER_TOO_SMALL {
        return None;
    }

    if layout_length == 0 || layout_length < 24 {
        // minimum size: layout_length(2) + guid(16) + offset(4) + count(1) + 1 descriptor
        return None;
    }

    // Allocate buffer for the keyboard layout structure
    // HiiKeyboardLayout: u16 layout_length + Guid + u32 layout_descriptor_string_offset + u8 descriptor_count + KeyDescriptor[]
    let total_size = layout_length as usize;
    let mut layout_buf = Vec::with_capacity(total_size);

    let status = unsafe {
        (hii_raw.get_keyboard_layout)(
            hii_raw,
            guid as *const _ as *const _,
            &mut layout_length,
            layout_buf.as_mut_ptr() as *mut _,
        )
    };

    if status.is_error() {
        return None;
    }

    // SAFETY: We allocated enough space
    unsafe {
        layout_buf.set_len(total_size);
    }

    // Parse the layout to get descriptor string offset
    // HiiKeyboardLayout structure:
    // - layout_length: u16 at offset 0
    // - guid: Guid at offset 2
    // - layout_descriptor_string_offset: u32 at offset 18 (2 + 16)
    // - descriptor_count: u8 at offset 22
    // - descriptors: KeyDescriptor[descriptor_count] at offset 23
    
    if layout_buf.len() < 24 {
        return None;
    }

    // layout_descriptor_string_offset is at offset 2+16=18
    let descriptor_offset = u32::from_le_bytes([
        layout_buf[18],
        layout_buf[19],
        layout_buf[20],
        layout_buf[21],
    ]) as usize;

    // Check if offset is valid
    if descriptor_offset >= layout_buf.len() {
        return None;
    }

    // The descriptor string is a null-terminated UTF-16LE string
    let desc_bytes = &layout_buf[descriptor_offset..];
    let (string, _) = read_utf16le_string(desc_bytes);
    
    if string.is_empty() {
        None
    } else {
        Some(string)
    }
}

/// Set the active keyboard layout by GUID.
///
/// This function:
/// 1. Activates the layout in firmware via HII Database Protocol
/// 2. Persists the GUID to NVRAM for automatic reload on next boot
pub fn set_active_keyboard_layout(guid: &Guid) -> Result<()> {
    // Find the layout in the list to get a nice status message
    let descriptor = if let Ok(layouts) = list_keyboard_layouts() {
        layouts.iter()
            .find(|l| l.guid == *guid)
            .map(|l| l.descriptor.clone())
            .unwrap_or_else(|| format!("Layout {:#010x}", u32::from_le_bytes(guid.to_bytes()[..4].try_into().unwrap_or([0,0,0,0]))))
    } else {
        format!("Layout {:#010x}", u32::from_le_bytes(guid.to_bytes()[..4].try_into().unwrap_or([0,0,0,0])))
    };
    
    uefi::println!("[keyboard_layout] Setting keyboard layout: {}", descriptor);

    if let Ok(handle) = get_handle_for_protocol::<HiiDatabase>() {
        if let Ok(mut hii_db) = open_protocol_exclusive::<HiiDatabase>(handle) {
            // Access the underlying raw protocol
            // SAFETY: HiiDatabase is #[repr(transparent)] wrapper around HiiDatabaseProtocol
            let hii_raw: &mut HiiDatabaseProtocol = unsafe {
                &mut *(&mut *hii_db as *mut HiiDatabase as *mut HiiDatabaseProtocol)
            };
            
            let status = unsafe {
                (hii_raw.set_keyboard_layout)(
                    hii_raw,
                    guid as *const _ as *const _,
                )
            };
            // Ignore errors - firmware might not support this
            if status.is_error() {
                uefi::println!("[keyboard_layout] set_keyboard_layout returned: {:?}", status);
            }
        }
    }

    // Save to NVRAM
    save_keyboard_layout_guid(guid)
}

/// Load the previously saved active keyboard layout GUID from NVRAM.
///
/// Returns `Ok(None)` if no layout has been saved yet.
pub fn load_active_keyboard_layout() -> Result<Option<Guid>> {
    // Create CStr16 from string
    let mut buf = [0u16; 32]; // More than enough for "KbdLayout" + nul
    let name = uefi::CStr16::from_str_with_buf(KBD_LAYOUT_NVRAM_NAME, &mut buf)
        .map_err(|_| uefi::Error::from(uefi::Status::INVALID_PARAMETER))?;

    let raw = get_settings_raw(name)?;
    let Some(raw) = raw else {
        uefi::println!("[keyboard_layout] No saved layout found in NVRAM");
        return Ok(None);
    };

    if raw.len() != 16 {
        // Invalid GUID length
        uefi::println!("[keyboard_layout] Invalid GUID length: {} bytes", raw.len());
        return Ok(None);
    }

    // Parse raw bytes as GUID
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&raw[..16]);
    // Create a new Guid from bytes
    let guid = Guid::from_bytes(bytes);
    
    uefi::println!("[keyboard_layout] Loaded layout GUID: {:#010x}...", 
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));

    Ok(Some(guid))
}

/// Save a keyboard layout GUID to NVRAM.
fn save_keyboard_layout_guid(guid: &Guid) -> Result<()> {
    // Create CStr16 from string
    let mut buf = [0u16; 32];
    let name = uefi::CStr16::from_str_with_buf(KBD_LAYOUT_NVRAM_NAME, &mut buf)
        .map_err(|_| uefi::Error::from(uefi::Status::INVALID_PARAMETER))?;

    // Convert GUID to bytes
    let bytes = guid.to_bytes();
    set_settings_raw(name, &bytes)
}

/// NVRAM vendor GUID for keyboard layout settings.
/// This uses the same vendor as other uefi_ui settings for consistency.
pub fn keyboard_layout_vendor() -> uefi::runtime::VariableVendor {
    SETTINGS_VENDOR
}

#[cfg(all(test, feature = "uefi"))]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_layout_picker_state() {
        let layouts = vec![
            KeyboardLayout {
                guid: Guid::null(),
                descriptor: String::from("US English"),
            },
            KeyboardLayout {
                guid: Guid::null(),
                descriptor: String::from("Danish"),
            },
        ];

        let mut state = KeyboardLayoutPickerState::new(layouts);
        assert_eq!(state.len(), 2);
        assert!(!state.is_empty());
        assert_eq!(state.selected, 0);

        assert_eq!(state.selected_layout().unwrap().descriptor, "US English");
        
        // Test selection
        state.selected = 1;
        assert_eq!(state.selected_layout().unwrap().descriptor, "Danish");
    }

    #[test]
    fn test_guid_null() {
        // Test that a null GUID is all zeros
        let guid = Guid::null();
        assert_eq!(guid.to_bytes(), [0u8; 16]);
    }

    #[test]
    fn test_utf16le_string() {
        // Test UTF-16LE string parsing
        // "Test" in UTF-16LE: T(0x54 0x00) e(0x65 0x00) s(0x73 0x00) t(0x74 0x00) + null(0x00 0x00)
        let data = b"T\x00e\x00s\x00t\x00\x00\x00";
        let (string, len) = read_utf16le_string(data);
        assert_eq!(string, "Test");
        assert_eq!(len, 10); // 4 chars * 2 bytes + 2 byte null
        
        // Empty string
        let data = b"\x00\x00";
        let (string, len) = read_utf16le_string(data);
        assert_eq!(string, "");
        assert_eq!(len, 2);
    }
}
