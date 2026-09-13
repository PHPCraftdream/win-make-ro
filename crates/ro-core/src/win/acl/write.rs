use std::path::Path;

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};
use windows::Win32::Security::{ACL, DACL_SECURITY_INFORMATION};
use windows::core::PCWSTR;

use crate::types::{Error, Result};

/// Replaces the DACL. Inheritance protection state is left as is; inheritable
/// ACEs are propagated to the subtree by Windows inside this call.
pub fn write_dacl(path: &Path, wide: &[u16], acl: *const ACL) -> Result<()> {
    // SAFETY: wide is NUL-terminated; acl is a valid, initialized ACL.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
    };
    if rc != ERROR_SUCCESS { Err(Error::win(path, rc)) } else { Ok(()) }
}
