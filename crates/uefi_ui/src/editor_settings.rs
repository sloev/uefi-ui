//! Editor and theme settings — serialized via [`crate::settings::SettingsBlob`] for UEFI NVRAM.
//!
//! On UEFI: use [`crate::uefi_vars`] to persist. On host / tests: use [`SettingsBlob`] directly.

use alloc::string::String;
use alloc::vec::Vec;
use crate::settings::SettingsBlob;

// Keys used in the SettingsBlob
const KEY_FONT_SIZE:   &[u8] = b"font_size";
const KEY_THEME_INDEX: &[u8] = b"theme_index";
const KEY_LAST_FILE:   &[u8] = b"last_file";
const KEY_LAST_DIR:    &[u8] = b"last_dir";

pub const DEFAULT_FONT_SIZE:   u8 = 14;
pub const DEFAULT_THEME_INDEX: u8 = 0;

/// Persistent settings for the text editor.
///
/// Stored in UEFI NVRAM as a [`SettingsBlob`] (version 1). All fields have safe defaults
/// so a missing or corrupt NV variable degrades gracefully.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorSettings {
    /// Font size in points (e.g. 10, 12, 14, 16, 20).
    pub font_size: u8,
    /// Index into the application's theme list.
    pub theme_index: u8,
    /// Last opened file name (not a full path — stored separately from last_dir).
    /// Empty string means no recent file.
    pub last_file: String,
    /// Last visited directory as slash-separated path components joined with '/'.
    /// Empty string means the root.
    pub last_dir: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            font_size: DEFAULT_FONT_SIZE,
            theme_index: DEFAULT_THEME_INDEX,
            last_file: String::new(),
            last_dir: String::new(),
        }
    }
}

impl EditorSettings {
    pub fn new() -> Self { Self::default() }

    /// Reconstruct last_dir as a path component Vec (split on '/').
    pub fn last_dir_path(&self) -> Vec<String> {
        if self.last_dir.is_empty() {
            return alloc::vec![];
        }
        self.last_dir.split('/').map(String::from).collect()
    }

    /// Store last_dir from a path component Vec.
    pub fn set_last_dir_path(&mut self, path: &[String]) {
        let mut s = String::new();
        for (i, p) in path.iter().enumerate() {
            if i > 0 { s.push('/'); }
            s.push_str(p);
        }
        self.last_dir = s;
    }

    /// Encode to a [`SettingsBlob`] (version 1).
    pub fn to_blob(&self) -> SettingsBlob {
        let mut pairs: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        pairs.push((KEY_FONT_SIZE.to_vec(), alloc::vec![self.font_size]));
        pairs.push((KEY_THEME_INDEX.to_vec(), alloc::vec![self.theme_index]));
        if !self.last_file.is_empty() {
            pairs.push((KEY_LAST_FILE.to_vec(), self.last_file.as_bytes().to_vec()));
        }
        if !self.last_dir.is_empty() {
            pairs.push((KEY_LAST_DIR.to_vec(), self.last_dir.as_bytes().to_vec()));
        }
        SettingsBlob { version: 1, pairs }
    }

    /// Decode from a [`SettingsBlob`]; unknown keys are ignored (forward compat).
    /// Returns `None` if the blob is wrong version.
    pub fn from_blob(blob: &SettingsBlob) -> Option<Self> {
        if blob.version != 1 {
            return None;
        }
        let mut s = Self::default();
        for (k, v) in &blob.pairs {
            if k == KEY_FONT_SIZE && !v.is_empty() {
                s.font_size = v[0];
            } else if k == KEY_THEME_INDEX && !v.is_empty() {
                s.theme_index = v[0];
            } else if k == KEY_LAST_FILE {
                s.last_file = String::from_utf8_lossy(v).into_owned();
            } else if k == KEY_LAST_DIR {
                s.last_dir = String::from_utf8_lossy(v).into_owned();
            }
        }
        Some(s)
    }

    /// Encode to raw bytes for NVRAM storage.
    pub fn encode(&self) -> Vec<u8> {
        self.to_blob().encode()
    }

    /// Decode from raw NVRAM bytes; corrupt / missing data returns `None`.
    pub fn decode(raw: &[u8]) -> Option<Self> {
        SettingsBlob::decode(raw).and_then(|b| Self::from_blob(&b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_full() {
        let s = EditorSettings {
            font_size: 16,
            theme_index: 2,
            last_file: String::from("notes.txt"),
            last_dir: String::from("docs/letters"),
        };
        let raw = s.encode();
        let s2 = EditorSettings::decode(&raw).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn roundtrip_defaults() {
        let s = EditorSettings::default();
        let raw = s.encode();
        let s2 = EditorSettings::decode(&raw).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn corrupt_data_returns_none() {
        assert!(EditorSettings::decode(b"garbage").is_none());
        assert!(EditorSettings::decode(b"").is_none());
    }

    #[test]
    fn last_dir_path_roundtrip() {
        let mut s = EditorSettings::default();
        let path = alloc::vec![String::from("docs"), String::from("letters")];
        s.set_last_dir_path(&path);
        assert_eq!(s.last_dir_path(), path);
    }

    #[test]
    fn empty_last_dir_path_is_empty_vec() {
        let s = EditorSettings::default();
        assert_eq!(s.last_dir_path(), alloc::vec![] as Vec<String>);
    }
}
