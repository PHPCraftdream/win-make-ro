use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows_registry::{Key, Result, Type};

/// Writes a path as `REG_SZ` from raw UTF-16.
///
/// `to_string_lossy` would fold an unpaired surrogate — a legal part of a
/// Windows path — into U+FFFD, and the registration would then name a file
/// that does not exist.
pub fn set_path(key: &Key, name: &str, path: &Path) -> Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let bytes: Vec<u8> = wide.iter().flat_map(|unit| unit.to_le_bytes()).collect();
    key.set_bytes(name, Type::String, &bytes)
}
