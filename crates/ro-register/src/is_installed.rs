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
    key.get_string("").ok().map(PathBuf::from)
}
