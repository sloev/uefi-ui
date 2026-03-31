//! Serialize settings to a byte blob for NVRAM (or any key-value store).

use alloc::vec::Vec;

pub const SETTINGS_MAGIC: u32 = 0x55_45_46_49; // "UEFI"

/// Simple versioned payload: magic, version, then key/value pairs (`u16` key len, bytes, `u16` val len, bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsBlob {
    pub version: u16,
    pub pairs: Vec<(Vec<u8>, Vec<u8>)>,
}

impl SettingsBlob {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&SETTINGS_MAGIC.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        let n = self.pairs.len().min(u16::MAX as usize) as u16;
        out.extend_from_slice(&n.to_le_bytes());
        for (k, v) in &self.pairs {
            let kl = k.len().min(u16::MAX as usize) as u16;
            let vl = v.len().min(u16::MAX as usize) as u16;
            out.extend_from_slice(&kl.to_le_bytes());
            out.extend_from_slice(&k[..kl as usize]);
            out.extend_from_slice(&vl.to_le_bytes());
            out.extend_from_slice(&v[..vl as usize]);
        }
        out
    }

    pub fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() < 8 {
            return None;
        }
        let magic = u32::from_le_bytes(raw[0..4].try_into().ok()?);
        if magic != SETTINGS_MAGIC {
            return None;
        }
        let version = u16::from_le_bytes(raw[4..6].try_into().ok()?);
        let n = u16::from_le_bytes(raw[6..8].try_into().ok()?);
        let mut i = 8usize;
        let mut pairs = Vec::new();
        for _ in 0..n {
            if i + 2 > raw.len() {
                return None;
            }
            let kl = u16::from_le_bytes(raw[i..i + 2].try_into().ok()?) as usize;
            i += 2;
            if i + kl > raw.len() {
                return None;
            }
            let k = raw[i..i + kl].to_vec();
            i += kl;
            if i + 2 > raw.len() {
                return None;
            }
            let vl = u16::from_le_bytes(raw[i..i + 2].try_into().ok()?) as usize;
            i += 2;
            if i + vl > raw.len() {
                return None;
            }
            let v = raw[i..i + vl].to_vec();
            i += vl;
            pairs.push((k, v));
        }
        Some(SettingsBlob { version, pairs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let s = SettingsBlob {
            version: 1,
            pairs: alloc::vec![(b"theme".to_vec(), b"dark".to_vec())],
        };
        let b = s.encode();
        assert_eq!(SettingsBlob::decode(&b), Some(s));
    }
}
