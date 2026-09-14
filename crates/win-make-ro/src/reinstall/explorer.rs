use std::process::Command;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, PostMessageW,
};
use windows::core::w;

use super::outcome::Restart;

/// `WM_USER + 436`, the message the shell's own hidden *Exit Explorer* entry
/// sends. Explorer saves its state and exits, and — unlike a process that was
/// killed — Windows does not bring it back, so the restart is ours to do and
/// happens exactly once.
const WM_EXIT_EXPLORER: u32 = 0x5B4;

/// How long Explorer is given to close. It is writing out window state, not
/// doing work that scales with anything, so this only has to outlast a busy
/// machine.
const EXIT_TIMEOUT_MS: u32 = 30_000;

/// Asks Explorer to exit, waits until it is gone, and starts it again.
///
/// The wait is the reason this exists: the DLL stays mapped until the process
/// that loaded it ends, so anything that wants to delete the superseded copy
/// has to know that moment has passed. Every way this can go wrong is reported
/// as itself — a second Explorer started beside a first one that was never
/// asked to leave is not a restart, and must never be called one.
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

    let shell = match shell() {
        Ok(s) => s,
        Err(e) => return Restart::Failed(e),
    };
    if let Some(shell) = shell {
        // SAFETY: the window is live; the message carries no pointers.
        if let Err(e) =
            unsafe { PostMessageW(Some(shell.window), WM_EXIT_EXPLORER, WPARAM(0), LPARAM(0)) }
        {
            return Restart::Failed(format!("could not ask Explorer to close: {e}"));
        }
        // SAFETY: the handle is owned by `shell` and live until it drops.
        if unsafe { WaitForSingleObject(shell.process, EXIT_TIMEOUT_MS) } != WAIT_OBJECT_0 {
            return Restart::Failed("Explorer did not close; nothing was restarted".into());
        }
    }
    match start() {
        Ok(()) => Restart::Restarted,
        Err(e) => Restart::Failed(e),
    }
}

/// The shell's tray window together with a handle to the process behind it.
struct Shell {
    window: HWND,
    process: HANDLE,
}

impl Drop for Shell {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess and is not used again.
        let _ = unsafe { CloseHandle(self.process) };
    }
}

/// `Ok(None)` only when there is no shell to close — a machine in that state
/// just needs the start. A handle that could not be opened is an error and not
/// an absence: starting a second Explorer beside a live one would leave the old
/// DLL mapped and look like success.
fn shell() -> Result<Option<Shell>, String> {
    // SAFETY: a static NUL-terminated class name and a null window name.
    let Ok(window) = (unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }) else {
        return Ok(None);
    };
    let mut pid = 0u32;
    // SAFETY: the window is live; pid is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return Err("found the shell window but not the process behind it".into());
    }
    // Opened before Explorer is told to go: afterwards the process may already
    // be gone and there would be nothing left to wait on.
    // SAFETY: pid names a live process; the handle is owned by `Shell`.
    match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(process) => Ok(Some(Shell { window, process })),
        Err(e) => Err(format!("could not open the Explorer process ({pid}): {e}")),
    }
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

/// Whether this process is running with an elevated token.
///
/// Fallible on purpose: a token that cannot be read is not evidence of an
/// ordinary one, and the cost of guessing wrong is an elevated desktop.
fn elevated() -> Result<bool, String> {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo-handle; token receives a real one.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }
        .map_err(|e| format!("cannot open this process's token: {e}"))?;
    let mut info = TOKEN_ELEVATION::default();
    let size = std::mem::size_of::<TOKEN_ELEVATION>() as u32;
    let mut written = 0u32;
    // SAFETY: the buffer is a TOKEN_ELEVATION and `size` is its length.
    let read = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            Some(std::ptr::from_mut(&mut info).cast()),
            size,
            &mut written,
        )
    };
    // SAFETY: token came from OpenProcessToken and is not used again.
    let _ = unsafe { CloseHandle(token) };
    read.map_err(|e| format!("cannot read the elevation of this process: {e}"))?;
    Ok(info.TokenIsElevated != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing here calls [`restart`]: it would close the desktop of whoever
    /// is running the tests. Only the two questions it asks first are checked,
    /// and both used to answer with a guess.
    ///
    /// The token either reads or says why it could not. Turning a failed read
    /// into "not elevated" was the dangerous direction: that is the one that
    /// goes on to start an Explorer holding administrator rights.
    #[test]
    fn the_elevation_of_this_process_can_be_read() {
        assert!(elevated().is_ok(), "{:?}", elevated().err());
    }

    /// Looking for the shell may find nothing — a session with no desktop,
    /// which is what a build agent is — but finding nothing and failing to
    /// look are different answers. They used to be the same one, and the
    /// caller then started a second Explorer beside a live one and called it a
    /// restart.
    #[test]
    fn looking_for_the_shell_either_finds_it_or_says_it_is_not_there() {
        let found = shell().expect("looking for the shell reported a failure");
        if let Some(shell) = found {
            assert!(!shell.process.is_invalid(), "found the shell but not a usable handle");
        }
    }
}
