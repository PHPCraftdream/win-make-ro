use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{CREATE_NO_WINDOW, WaitForSingleObject};

use super::elevated::elevated;
use crate::cli::Command as Verb;
use crate::reinstall::outcome::Restart;

/// How long the caller is kept waiting before being told the exchange is still
/// going. Ending this wait ends nothing else: the child holds the request and
/// the duty to start the shell again for as long as it takes.
const REPORT_AFTER_MS: u32 = 30_000;

/// Restarts Explorer, in a child process that cannot be talked out of it.
///
/// The request to close cannot be withdrawn once it is queued, so whoever
/// sends it owes the start until the shell has actually gone. That duty is
/// given to a child with no deadline; this function only watches, and when it
/// stops watching it says so rather than calling the work finished or failed.
pub fn restart() -> Restart {
    match elevated() {
        Ok(true) => {
            return Restart::Skipped(
                "this process is elevated, and the new Explorer would keep those rights \
                 for the whole session"
                    .into(),
            );
        }
        Err(e) => {
            return Restart::Skipped(format!(
                "could not tell whether this process is elevated ({e}), and starting an \
                 elevated Explorer is not a risk worth taking on a guess"
            ));
        }
        Ok(false) => {}
    }

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return Restart::Failed(format!("cannot locate own executable: {e}")),
    };
    // Detached from this console: it outlives the command, and by then there
    // may be nothing left to write to.
    let spawned = Command::new(exe)
        .arg(Verb::RestartExplorer.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW.0)
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        // Nothing has been asked of Explorer yet, so nothing is left hanging.
        Err(e) => return Restart::Failed(format!("cannot start the Explorer restart: {e}")),
    };

    let handle = HANDLE(child.as_raw_handle());
    // SAFETY: the handle belongs to `child`, which outlives this call.
    if unsafe { WaitForSingleObject(handle, REPORT_AFTER_MS) } != WAIT_OBJECT_0 {
        return Restart::Pending(format!(
            "Explorer has not closed yet; it will be started again as soon as it does \
             (process {})",
            child.id()
        ));
    }
    match child.wait() {
        Ok(status) if status.success() => Restart::Restarted,
        Ok(status) => Restart::Failed(format!(
            "the Explorer restart gave up with exit code {}",
            status.code().unwrap_or(-1)
        )),
        Err(e) => Restart::Failed(format!("lost track of the Explorer restart: {e}")),
    }
}
