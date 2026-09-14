use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
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

/// The running shell: its tray window and a handle to the process behind it.
///
/// The handle is opened while the process is certainly alive, before anything
/// is asked of it. Waiting on a handle rather than looking a process id up
/// again is what makes the wait mean this shell and not whatever else Windows
/// gives that number to next.
pub struct Shell {
    window: HWND,
    process: HANDLE,
}

impl Drop for Shell {
    fn drop(&mut self) {
        // SAFETY: the handle came from OpenProcess and is not used again.
        let _ = unsafe { CloseHandle(self.process) };
    }
}

impl Shell {
    /// `Ok(None)` only when there is no shell running — a session in that state
    /// just needs one started. A handle that could not be opened is an error
    /// and not an absence: treating it as one would mean starting a second
    /// Explorer beside a live one and calling that a restart.
    pub fn find() -> Result<Option<Self>, String> {
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
        // SAFETY: pid names a live process; the handle is owned by `Shell`.
        match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
            Ok(process) => Ok(Some(Self { window, process })),
            Err(e) => Err(format!("could not open the Explorer process ({pid}): {e}")),
        }
    }

    /// Queues the request to close. It is a request, not an order, and it is
    /// not withdrawable: from here on somebody has to be waiting to start the
    /// shell again, whatever else happens.
    pub fn ask_to_close(&self) -> Result<(), String> {
        // SAFETY: the window is live; the message carries no pointers.
        unsafe { PostMessageW(Some(self.window), WM_EXIT_EXPLORER, WPARAM(0), LPARAM(0)) }
            .map_err(|e| format!("could not ask Explorer to close: {e}"))
    }

    /// Blocks until the process ends. `Err` means it is still running, which
    /// is the one state in which not starting another shell is safe.
    pub fn wait_until_gone(&self, timeout_ms: u32) -> Result<(), String> {
        // SAFETY: the handle is owned by self and live until it drops.
        if unsafe { WaitForSingleObject(self.process, timeout_ms) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        Err(format!("Explorer was still running {} seconds later", timeout_ms / 1000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Looking for the shell may find nothing — a session with no desktop,
    /// which is what a build agent is — but finding nothing and failing to
    /// look are different answers. They used to be the same one, and the
    /// caller then started a second Explorer beside a live one and called it a
    /// restart. Nothing here asks anything to close.
    #[test]
    fn looking_for_the_shell_either_finds_it_or_says_it_is_not_there() {
        let found = Shell::find().expect("looking for the shell reported a failure");
        if let Some(shell) = found {
            assert!(!shell.process.is_invalid(), "found the shell but not a usable handle");
        }
    }
}
