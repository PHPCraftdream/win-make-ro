use std::io;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use super::Item;

/// Set to `1` to make the DLL wait for the helper (used by e2e tests).
pub const SYNC_ENV: &str = "WIN_MAKE_RO_SYNC";

/// Starts `win-make-ro.exe <lock|unlock> --gui -- <paths...>` without a console.
pub fn launch(helper: &Path, item: Item, paths: &[PathBuf]) -> io::Result<()> {
    let cmd = item.command().ok_or_else(|| io::Error::other("item has no command"))?;
    let mut child = Command::new(helper)
        .arg(cmd)
        .arg("--gui")
        .arg("--")
        .args(paths)
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn()?;
    if std::env::var_os(SYNC_ENV).is_some_and(|v| v == "1") {
        child.wait()?;
    }
    Ok(())
}
