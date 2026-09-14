use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

/// Whether this process is running with an elevated token.
///
/// Fallible on purpose: a token that cannot be read is not evidence of an
/// ordinary one, and the cost of guessing wrong is a desktop that hands
/// administrator rights to everything started from it for the rest of the
/// session.
pub fn elevated() -> Result<bool, String> {
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
    use super::elevated;

    /// The token either reads or says why it could not. Turning a failed read
    /// into "not elevated" was the dangerous direction: that is the one that
    /// goes on to start an Explorer holding administrator rights.
    #[test]
    fn the_elevation_of_this_process_can_be_read() {
        assert!(elevated().is_ok(), "{:?}", elevated().err());
    }
}
