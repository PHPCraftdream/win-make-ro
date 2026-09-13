use std::path::Path;

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};
use windows::Win32::Security::DACL_SECURITY_INFORMATION;
use windows::core::PCWSTR;

use crate::types::{Error, Result};

/// Restores a NULL DACL (everyone gets every right). Not the same as an empty
/// DACL, which denies everyone everything — see
/// <https://learn.microsoft.com/en-us/windows/win32/secauthz/null-dacls-and-empty-dacls>.
pub fn write_null_dacl(path: &Path, wide: &[u16]) -> Result<()> {
    // SAFETY: wide is NUL-terminated; a None DACL pointer means "NULL DACL".
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
        )
    };
    if rc == ERROR_SUCCESS { Ok(()) } else { Err(Error::win(path, rc)) }
}
