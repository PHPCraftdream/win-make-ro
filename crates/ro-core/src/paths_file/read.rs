use std::ffi::OsString;
use std::io;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

/// Reads a list written by `write_paths_file`.
///
/// An empty file yields no paths rather than one empty path, so a selection
/// that turned out to be empty does not become an operation on "".
pub fn read_paths_file(path: &Path) -> io::Result<Vec<PathBuf>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() % 2 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "the path list is not whole UTF-16 code units",
        ));
    }
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
    Ok(units.split(|&u| u == 0).map(|entry| PathBuf::from(OsString::from_wide(entry))).collect())
}
