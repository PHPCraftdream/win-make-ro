use windows_registry::{CURRENT_USER, Key, Result};

use crate::clsid::{CLSID, HANDLER_NAME};
use crate::install::{CLASSES, TARGETS};
use crate::notify::notify;

/// Removes every key written by `install`. Missing keys are not an error.
pub fn uninstall() -> Result<()> {
    uninstall_under(&CURRENT_USER.create(CLASSES)?)?;
    notify();
    Ok(())
}

pub fn uninstall_under(classes: &Key) -> Result<()> {
    for t in TARGETS {
        ignore_missing(
            classes.remove_tree(format!(r"{t}\shellex\ContextMenuHandlers\{HANDLER_NAME}")),
        )?;
    }
    ignore_missing(classes.remove_tree(format!(r"CLSID\{CLSID}")))
}

fn ignore_missing(r: Result<()>) -> Result<()> {
    const ERROR_FILE_NOT_FOUND: i32 = 2;
    match r {
        Err(e) if e.code().0 & 0xFFFF == ERROR_FILE_NOT_FOUND => Ok(()),
        r => r,
    }
}
