use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// `\\?\`-prefixed, NUL-terminated UTF-16 absolute path.
pub fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let abs = std::path::absolute(path)?;
    let raw: Vec<u16> = abs.as_os_str().encode_wide().collect();
    let bs = b'\\' as u16;
    let mut v: Vec<u16> = if raw.starts_with(&[bs, bs, b'?' as u16]) {
        raw
    } else if raw.starts_with(&[bs, bs]) {
        // UNC: \\server\share -> \\?\UNC\server\share
        let mut v: Vec<u16> = OsStr::new(r"\\?\UNC\").encode_wide().collect();
        v.extend_from_slice(&raw[2..]);
        v
    } else {
        let mut v: Vec<u16> = OsStr::new(r"\\?\").encode_wide().collect();
        v.extend_from_slice(&raw);
        v
    };
    v.push(0);
    Ok(v)
}
