use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Whether this process is running with an elevated token.
///
/// It matters here because a process starts Explorer with its own token, and an
/// elevated shell would hand administrator rights to everything launched from
/// the desktop for the rest of the session.
pub fn elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: GetCurrentProcess returns a pseudo-handle; token receives a real one.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
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
    }
    .is_ok();
    // SAFETY: token came from OpenProcessToken and is not used again.
    let _ = unsafe { CloseHandle(token) };
    read && info.TokenIsElevated != 0
}
