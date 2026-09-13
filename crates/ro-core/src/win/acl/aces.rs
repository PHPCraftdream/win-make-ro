use windows::Win32::Security::{ACE_HEADER, ACL, GetAce};

use super::AceRef;

/// All ACEs of `acl` in order; empty for a NULL DACL.
pub fn aces(acl: *const ACL) -> Vec<AceRef> {
    if acl.is_null() {
        return Vec::new();
    }
    // SAFETY: acl is a valid ACL returned by GetNamedSecurityInfoW.
    let count = unsafe { (*acl).AceCount };
    (0..u32::from(count))
        .filter_map(|i| {
            let mut p: *mut core::ffi::c_void = std::ptr::null_mut();
            // SAFETY: index < AceCount.
            unsafe { GetAce(acl, i, &mut p) }.ok()?;
            Some(AceRef { ptr: p as *const ACE_HEADER })
        })
        .collect()
}
