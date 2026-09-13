use std::path::Path;

use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL};
use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
use windows::Win32::Security::{ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR};
use windows::core::PCWSTR;

use crate::types::{Error, Result};
use crate::win::LocalBuf;

/// Owned security descriptor + borrowed DACL pointer (null = NULL DACL).
pub struct Dacl {
    _sd: LocalBuf,
    pub acl: *mut ACL,
}

impl Dacl {
    pub fn read(path: &Path, wide: &[u16]) -> Result<Dacl> {
        let mut sd = PSECURITY_DESCRIPTOR::default();
        let mut acl: *mut ACL = std::ptr::null_mut();
        // SAFETY: wide is NUL-terminated; out-pointers are valid for writes.
        let rc = unsafe {
            GetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(&mut acl),
                None,
                &mut sd,
            )
        };
        if rc != ERROR_SUCCESS {
            return Err(Error::win(path, rc));
        }
        Ok(Dacl { _sd: LocalBuf(HLOCAL(sd.0)), acl })
    }
}
