# Execution Plan: Keyboard Layout Support

## Overview

Add keyboard layout selection and persistence to the uefi-ui crate, then integrate it into lotus-os.

**Rationale:** Danish keyboard is seen as English in UEFI (VirtualBox). UEFI firmware provides keyboard layout management via HII Database Protocol.

---

## Phase 1: Core Implementation in uefi-ui crate

### File: `crates/uefi_ui/src/keyboard_layout.rs` (NEW)

**Purpose:** General keyboard layout management usable by any UEFI app (not just lotus-os).

#### Dependencies
- `uefi-raw` 0.14+ for HII Database Protocol raw bindings
- `uefi` 0.37+ for NVRAM access
- `alloc` for `Vec`

#### Types

```rust
/// Unique identifier for a keyboard layout
pub type KeyboardLayoutGuid = Guid;

/// Keyboard layout descriptor with human-readable metadata
#[derive(Debug, Clone)]
pub struct KeyboardLayout {
    pub guid: Guid,
    pub descriptor_string: String,  // e.g., "Danish", "US English"
    pub is_active: bool,
}

/// State for keyboard layout picker dialog
#[derive(Debug, Clone)]
pub struct KeyboardLayoutPickerState {
    pub layouts: Vec<KeyboardLayout>,
    pub selected: usize,
    pub scroll_top: usize,
}

/// NVRAM storage key for active keyboard layout
pub const KBD_LAYOUT_NVRAM_NAME: &str = "KeyboardLayout";
```

#### Functions

1. **`list_keyboard_layouts()` -> `Result<Vec<KeyboardLayout>>`**
   - Use `HiiDatabaseProtocol::find_keyboard_layouts` to list all available layouts
   - For each GUID, use `get_keyboard_layout` to retrieve descriptor string
   - Mark current active layout
   - Returns empty vec if not supported by firmware

2. **`set_active_keyboard_layout(guid: &Guid) -> Result<()>`**
   - Use `HiiDatabaseProtocol::set_keyboard_layout` to activate
   - Save to NVRAM using `uefi_vars::set_settings_raw`
   - Store as raw GUID bytes (16 bytes)

3. **`load_active_keyboard_layout() -> Result<Option<Guid>>`**
   - Load from NVRAM using `uefi_vars::get_settings_raw`
   - Parse 16 bytes as GUID
   - Return `None` if not set

4. **`KeyboardLayoutPickerState` methods:**
   - `new()` -> Self
   - `reload()` -> reload layouts from firmware
   - `select_index(idx: usize)`
   - `confirm_selection()` -> applies and persists selection
   - `cancel()`

#### Integration with lib.rs
- Export `keyboard_layout` module in `lib.rs`
- Add `keyboard_layout` to the public API

---

## Phase 2: UI Components in uefi-ui crate

### File: `crates/uefi_ui/src/bedrock_controls.rs` (MODIFY)

**Add after `FilePickerLayout`:**

```rust
// ── Keyboard Layout Picker layout + draw ──────────────────────────────

/// Computed geometry for keyboard layout picker dialog
#[derive(Debug, Clone, Copy)]
pub struct KeyboardLayoutPickerLayout {
    pub dialog: Rectangle,
    pub title_bar: Rectangle,
    pub close_btn: Rectangle,
    pub list_outer: Rectangle,
    pub list_inner: Rectangle,
    pub sb_rect: Rectangle,
    pub ok_btn: Rectangle,
    pub cancel_btn: Rectangle,
    pub visible_rows: usize,
}

/// Constants for keyboard layout picker
pub const KBD_SB_W: u32 = 26;
pub const KBD_TITLE_H: u32 = 26;
pub const KBD_BUTTON_ROW_H: u32 = 38;
pub const KBD_BTN_W: u32 = 80;

pub fn compute_keyboard_layout_picker_layout(
    dialog: Rectangle,
    line_h: i32,
    sb_w: u32,
) -> KeyboardLayoutPickerLayout

pub fn draw_keyboard_layout_picker<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    bevel: &BedrockBevel,
    state: &KeyboardLayoutPickerState,
    layout: &KeyboardLayoutPickerLayout,
    theme: &Theme,
) -> Result<(), D::Error>
```

---

## Phase 3: Settings Menu Integration in lotus-os

### File: `crates/lotus_os/src/main.rs` (MODIFY)

1. **Add imports:**
```rust
use uefi_ui::keyboard_layout::{KeyboardLayoutPickerState, list_keyboard_layouts, 
                                set_active_keyboard_layout, load_active_keyboard_layout};
use uefi_ui::bedrock_controls::{KeyboardLayoutPickerLayout, compute_keyboard_layout_picker_layout,
                                 draw_keyboard_layout_picker, KBD_SB_W};
```

2. **Add Settings menu:**
```rust
// Add to MENU_LABELS
const MENU_LABELS: &[&str] = &["&File", "&Edit", "&Settings"];

// Add Settings menu items
const SETTINGS_MENU: &[&str] = &[
    "&Keyboard Layout",
];
const SETTINGS_IDX_KBD_LAYOUT: usize = 0;
```

3. **Add Mode variant:**
```rust
#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Editing,
    MenuBar,
    // ... existing modes
    SettingsMenu,      // Settings dropdown open
    KeyboardLayout,   // Keyboard layout picker modal
}
```

4. **Add state:**
```rust
// In main()
let mut kbd_layout_picker: Option<KeyboardLayoutPickerState> = None;
let mut kbd_layout_sb = ScrollbarState::new(ScrollAxis::Vertical, 1, 1, 0);
```

5. **Handle Settings menu navigation:**
```rust
// When Settings menu opens
Mode::SettingsMenu => {
    match ev.key {
        Key::Character('k') => {
            // Open keyboard layout picker
            if let Ok(layouts) = list_keyboard_layouts() {
                let mut picker = KeyboardLayoutPickerState::new(layouts);
                // Find current active layout and select it
                if let Ok(Some(active_guid)) = load_active_keyboard_layout() {
                    if let Some(idx) = picker.layouts.iter().position(|l| l.guid == active_guid) {
                        picker.selected = idx;
                    }
                }
                kbd_layout_picker = Some(picker);
                mode = Mode::KeyboardLayout;
            }
        }
        // ... other settings items
    }
}
```

6. **Handle Keyboard Layout picker mode:**
```rust
Mode::KeyboardLayout => {
    let mut picker = kbd_layout_picker.take().unwrap();
    match ev.key {
        Key::Enter => {
            // Apply selection
            if let Some(layout) = picker.layouts.get(picker.selected) {
                if let Err(e) = set_active_keyboard_layout(&layout.guid) {
                    status = format!("Failed to set layout: {:?}", e);
                } else {
                    status = format!("Keyboard layout set to: {}", layout.descriptor_string);
                }
            }
            mode = Mode::Editing;
        }
        Key::Escape => {
            mode = Mode::Editing;
        }
        Key::Up => {
            picker.selected = picker.selected.saturating_sub(1);
            kbd_layout_sb.set_position(picker.selected);
        }
        Key::Down => {
            picker.selected = (picker.selected + 1).min(picker.layouts.len().saturating_sub(1));
            kbd_layout_sb.set_position(picker.selected);
        }
        _ => {}
    }
    kbd_layout_picker = Some(picker);
    dirty = true;
}
```

7. **Add rendering for keyboard layout picker:**
```rust
// In render loop
if mode == Mode::KeyboardLayout {
    let picker = kbd_layout_picker.as_ref().unwrap();
    letdialog_rect = center_in_screen(Size::new(300, 200), screen_rect);
    let layout = compute_keyboard_layout_picker_layout(dialog_rect, LINE_H, SB_W);
    let sb = ScrollbarState::new(
        ScrollAxis::Vertical,
        picker.layouts.len().max(1),
        layout.visible_rows.max(1),
        picker.scroll_top,
    );
    draw_keyboard_layout_picker(&mut fb, &bevel, picker, &layout, &sb, &theme)?;
}
```

---

## Phase 4: Auto-load on boot

In `main.rs`, after initialization:

```rust
// Try to load and apply saved keyboard layout
if let Ok(Some(guid)) = load_active_keyboard_layout() {
    let _ = set_active_keyboard_layout(&guid);
}
```

---

## File Structure Summary

```
plans/
  keyboard_layout.md          # This plan

crates/uefi_ui/
  src/
    keyboard_layout.rs        # NEW: Core layout management
    bedrock_controls.rs      # MODIFY: Add picker UI
    lib.rs                    # MODIFY: Export keyboard_layout module

crates/lotus_os/
  src/main.rs               # MODIFY: Add settings menu + integration
```

---

## Dependencies Check

Check that `Cargo.toml` for uefi_ui has:
- `uefi-raw = { version = "0.14", default-features = false }` ✓ (already present)

No additional dependencies needed.

---

## Testing Strategy

1. Unit tests for `KeyboardLayout` encoding/decoding
2. Effect tests for NVRAM persistence
3. Manual testing in VirtualBox with various keyboard layouts

---

## Tasks Checklist

- [ ] Create `keyboard_layout.rs` with core types and functions
- [ ] Implement HII Database Protocol interaction
- [ ] Implement NVRAM persistence
- [ ] Add keyboard layout picker UI to bedrock_controls
- [ ] Export from lib.rs
- [ ] Add Settings menu to lotus-os
- [ ] Add keyboard layout picker mode handling
- [ ] Add auto-load on boot
- [ ] Test in VirtualBox
