use std::process::Command;

use windows::Win32::Foundation::{CloseHandle, HANDLE, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowThreadProcessId, PostMessageW,
};
use windows::core::w;

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
/// has to know that moment has passed.
pub fn restart() -> Result<(), String> {
    if let Some((window, process)) = shell() {
        // SAFETY: `window` is the live tray window; the message takes no pointers.
        unsafe { PostMessageW(Some(window), WM_EXIT_EXPLORER, WPARAM(0), LPARAM(0)) }
            .map_err(|e| format!("could not ask Explorer to close: {e}"))?;
        // SAFETY: `process` is a handle we opened and close below.
        let waited = unsafe { WaitForSingleObject(process, EXIT_TIMEOUT_MS) };
        // SAFETY: same handle, not used afterwards.
        let _ = unsafe { CloseHandle(process) };
        if waited != WAIT_OBJECT_0 {
            return Err("Explorer did not close; nothing was restarted".into());
        }
    }
    start()
}

/// The shell's tray window and a handle to the process behind it, if a shell
/// is running at all. A machine with no shell only needs the start.
fn shell() -> Option<(windows::Win32::Foundation::HWND, HANDLE)> {
    // SAFETY: both arguments are static NUL-terminated strings or null.
    let window = unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.ok()?;
    let mut pid = 0u32;
    // SAFETY: window is live; pid is a valid out-parameter.
    unsafe { GetWindowThreadProcessId(window, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    // The handle is opened before Explorer is told to go: afterwards the
    // process may already be gone and there would be nothing left to wait on.
    // SAFETY: pid names a live process; the handle is closed by the caller.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) }.ok()?;
    Some((window, process))
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
