use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;

use windows_registry::{CURRENT_USER, Key};

use crate::clsid::CLSID;
use crate::install::CLASSES;

/// DLL path currently registered for our CLSID, if any.
pub fn is_installed() -> Option<PathBuf> {
    is_installed_under(&CURRENT_USER.open(CLASSES).ok()?)
}

pub fn is_installed_under(classes: &Key) -> Option<PathBuf> {
    let key = classes.open(format!(r"CLSID\{CLSID}\InprocServer32")).ok()?;
    // Read as raw UTF-16 for the same reason it is written that way.
    let value = key.get_value("").ok()?;
    let wide = value.as_wide();
    let end = wide.iter().position(|&unit| unit == 0).unwrap_or(wide.len());
    Some(PathBuf::from(OsString::from_wide(&wide[..end])))
}
