use windows::Win32::Foundation::{HLOCAL, LocalFree};

/// Owns a `LocalAlloc`-allocated buffer returned by advapi32.
pub struct LocalBuf(pub HLOCAL);

impl Drop for LocalBuf {
    fn drop(&mut self) {
        if !self.0.0.is_null() {
            // SAFETY: handle came from an advapi32 allocation owned by us.
            unsafe { LocalFree(Some(self.0)) };
        }
    }
}
