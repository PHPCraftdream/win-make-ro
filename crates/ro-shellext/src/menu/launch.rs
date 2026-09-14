use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::System::Threading::CREATE_NO_WINDOW;

use super::Item;

/// Set to `1` to make the DLL wait for the helper (used by e2e tests).
pub const SYNC_ENV: &str = "WIN_MAKE_RO_SYNC";

/// Starts `win-make-ro.exe <lock|unlock> --gui -- <paths...>` without a console.
///
/// Explorer hands over whatever the user selected, and a few hundred long
/// names already exceed what a command line can carry. Past that point the
/// list travels in a file, which the helper reads and removes.
pub fn launch(helper: &Path, item: Item, paths: &[PathBuf]) -> io::Result<()> {
    let cmd = item.command().ok_or_else(|| io::Error::other("item has no command"))?;
    let mut command = Command::new(helper);
    command.arg(cmd).arg("--gui");

    let fixed = helper.as_os_str().encode_wide().count() + 64;
    let mut listed: Option<PathBuf> = None;
    if ro_core::fits_command_line(fixed, paths) {
        command.arg("--").args(paths);
    } else {
        let file = ro_core::write_paths_file(paths)?;
        // Handed over, not lent: nothing here can wait for the helper, so the
        // helper is the one that removes it.
        command.arg("--consume-paths-from").arg(&file);
        listed = Some(file);
    }

    let spawned = command.creation_flags(CREATE_NO_WINDOW.0).spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            // Nothing will read the list now, so it would sit in the temp
            // directory for good.
            if let Some(file) = &listed {
                ro_core::remove_paths_file(file);
            }
            return Err(e);
        }
    };
    if std::env::var_os(SYNC_ENV).is_some_and(|v| v == "1") {
        child.wait()?;
    }
    Ok(())
}
