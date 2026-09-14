use std::path::Path;

use windows_registry::{CURRENT_USER, Key, Result};

use crate::clsid::{CLSID, HANDLER_NAME};
use crate::notify::notify;
use crate::set_path::set_path;

pub(crate) const CLASSES: &str = r"Software\Classes";
pub(crate) const TARGETS: [&str; 2] = ["*", "Directory"];

/// Registers `dll` as an in-process context-menu handler for files and folders.
pub fn install(dll: &Path) -> Result<()> {
    install_under(&CURRENT_USER.create(CLASSES)?, dll)?;
    notify();
    Ok(())
}

/// Same, relative to an arbitrary `Classes`-shaped key (tests use a scratch key).
pub fn install_under(classes: &Key, dll: &Path) -> Result<()> {
    let clsid = classes.create(format!(r"CLSID\{CLSID}"))?;
    clsid.set_string("", "WinMakeRO context menu")?;
    let inproc = clsid.create("InprocServer32")?;
    set_path(&inproc, "", dll)?;
    inproc.set_string("ThreadingModel", "Apartment")?;
    for t in TARGETS {
        let k = classes.create(format!(r"{t}\shellex\ContextMenuHandlers\{HANDLER_NAME}"))?;
        k.set_string("", CLSID)?;
    }
    Ok(())
}
