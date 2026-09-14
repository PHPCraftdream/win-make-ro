use std::process::Command;

use super::elevated::elevated;
use super::shell::Shell;

/// How long the shell is given to close. Nothing here scales with anything, so
/// this only has to outlast a machine that is very busy indeed. It is the
/// whole budget: whoever runs this owes the start until the shell has actually
/// gone, and a wait that ends early ends with the old shell still running,
/// which is the one state where starting nothing is right.
const CLOSE_TIMEOUT_MS: u32 = 600_000;

/// Asks Explorer to close, waits until it has, and starts it again.
///
/// This is the whole exchange in one place on purpose. `PostMessageW` queues
/// the request and returns; nothing can take it back. Splitting the asking
/// from the waiting means a timeout on the waiting side abandons a request
/// that is still live, and an Explorer that closes a moment later leaves the
/// machine with no shell at all. So the process that asks is the process that
/// waits and starts, and it has no way of giving that up — which is why
/// `super::restart` runs this as a child and merely watches it.
pub fn close_and_start() -> Result<(), String> {
    // Checked here as well as in the caller, because this is reachable as its
    // own command: Explorer would inherit this process's token.
    match elevated() {
        Ok(false) => {}
        Ok(true) => {
            return Err("refusing to start Explorer from an elevated process: it would keep \
                        those rights for the whole session"
                .into());
        }
        Err(e) => return Err(format!("cannot tell whether this process is elevated: {e}")),
    }

    if let Some(shell) = Shell::find()? {
        shell.ask_to_close()?;
        shell.wait_until_gone(CLOSE_TIMEOUT_MS)?;
    }
    start()
}

fn start() -> Result<(), String> {
    // Named in full rather than left to PATH: with no shell running this is
    // the one thing that has to be found.
    let exe = std::env::var_os("SystemRoot")
        .map(|root| std::path::Path::new(&root).join("explorer.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| std::path::PathBuf::from("explorer.exe"));
    Command::new(exe)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Explorer was closed but could not be started again: {e}"))
}
