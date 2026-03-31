//! List directories on a [`uefi::proto::media::fs::SimpleFileSystem`] volume (FAT).

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use uefi::boot::find_handles;
use uefi::proto::media::file::{
    Directory, File, FileAttribute, FileMode, RegularFile,
};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;
use uefi::Error;
use uefi::Status;
use uefi::Result;

use crate::file_picker::{DirEntry, FileIo};

/// All handles that expose **Simple File System** (FAT volumes). Pair with
/// [`open_protocol_exclusive`](uefi::boot::open_protocol_exclusive) to browse files.
pub fn list_simple_fs_handles() -> Vec<uefi::Handle> {
    find_handles::<SimpleFileSystem>().unwrap_or_default()
}

/// Open `path` (slash-separated components, empty = volume root) as a directory handle.
pub fn open_directory_at_path(fs: &mut SimpleFileSystem, path: &[String]) -> Result<Directory> {
    let mut dir = fs.open_volume()?;
    for comp in path {
        let name =
            CString16::try_from(comp.as_str()).map_err(|_| Error::from(Status::INVALID_PARAMETER))?;
        let fh = dir.open(&name, FileMode::Read, FileAttribute::empty())?;
        dir = fh
            .into_directory()
            .ok_or_else(|| Error::from(Status::NOT_FOUND))?;
    }
    Ok(dir)
}

fn list_directory_entries(dir: &mut Directory) -> Result<Vec<DirEntry>> {
    let mut out = Vec::new();
    loop {
        let ent = match dir.read_entry_boxed()? {
            Some(e) => e,
            None => break,
        };
        let name = format!("{}", ent.file_name());
        if name.is_empty() {
            continue;
        }
        let is_dir = ent.attribute().contains(FileAttribute::DIRECTORY);
        out.push(DirEntry { name, is_dir });
    }
    Ok(out)
}

/// Read all entries in the **root** of `fs` into [`DirEntry`] (includes `.` / `..` handling left to UI).
pub fn list_root_directory(fs: &mut SimpleFileSystem) -> Result<Vec<DirEntry>> {
    let mut dir = fs.open_volume()?;
    list_directory_entries(&mut dir)
}

fn read_all_file(file: &mut RegularFile) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
}

/// [`FileIo`] over a single FAT volume ([`SimpleFileSystem`]) for use with [`crate::file_picker::FilePickerState`].
pub struct SimpleFsIo<'a> {
    pub fs: &'a mut SimpleFileSystem,
}

impl FileIo for SimpleFsIo<'_> {
    type Error = Error;

    fn list(&mut self, path: &[String]) -> core::result::Result<Vec<DirEntry>, Self::Error> {
        let mut dir = open_directory_at_path(self.fs, path)?;
        list_directory_entries(&mut dir)
    }

    fn read_file(
        &mut self,
        path: &[String],
        name: &str,
    ) -> core::result::Result<Vec<u8>, Self::Error> {
        let mut dir = open_directory_at_path(self.fs, path)?;
        let fname = CString16::try_from(name).map_err(|_| Error::from(Status::INVALID_PARAMETER))?;
        let fh = dir.open(&fname, FileMode::Read, FileAttribute::empty())?;
        let mut file = fh
            .into_regular_file()
            .ok_or_else(|| Error::from(Status::INVALID_PARAMETER))?;
        read_all_file(&mut file)
    }

    fn write_file(
        &mut self,
        path: &[String],
        name: &str,
        data: &[u8],
    ) -> core::result::Result<(), Self::Error> {
        let mut dir = open_directory_at_path(self.fs, path)?;
        let fname = CString16::try_from(name).map_err(|_| Error::from(Status::INVALID_PARAMETER))?;
        let fh = dir.open(&fname, FileMode::CreateReadWrite, FileAttribute::empty())?;
        let mut file = fh
            .into_regular_file()
            .ok_or_else(|| Error::from(Status::INVALID_PARAMETER))?;
        file
            .write(data)
            .map_err(|_| Error::from(Status::DEVICE_ERROR))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Calls UEFI boot services; only valid when a system table is installed (firmware / `uefi-test-runner`).
    #[test]
    #[ignore = "requires UEFI runtime; run with cargo test -p uefi_ui --features uefi -- --ignored on device or uefi-test-runner"]
    fn list_simple_fs_handles_smoke() {
        let v = list_simple_fs_handles();
        let _ = v.len();
    }
}
