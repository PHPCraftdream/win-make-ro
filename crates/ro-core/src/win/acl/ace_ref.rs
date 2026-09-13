use windows::Win32::Security::{ACCESS_ALLOWED_ACE, ACE_HEADER, EqualSid, INHERITED_ACE, PSID};

use super::DENY_TYPE;
use crate::types::LOCK_MASK;
use crate::win::Sid;

/// Borrowed view of one ACE inside a live DACL.
#[derive(Clone, Copy)]
pub struct AceRef {
    pub ptr: *const ACE_HEADER,
}

impl AceRef {
    pub fn header(&self) -> ACE_HEADER {
        // SAFETY: ptr points at a valid ACE inside a live ACL.
        unsafe { *self.ptr }
    }

    pub fn inherited(&self) -> bool {
        u32::from(self.header().AceFlags) & INHERITED_ACE.0 != 0
    }

    /// True for a lock ACE (explicit or inherited): deny, Everyone, exact mask.
    pub fn is_lock(&self, everyone: &Sid) -> bool {
        if self.header().AceType != DENY_TYPE {
            return false;
        }
        // ACCESS_DENIED_ACE shares ACCESS_ALLOWED_ACE's layout.
        // SAFETY: a deny ACE is always at least ACCESS_ALLOWED_ACE-sized.
        let ace = unsafe { &*(self.ptr as *const ACCESS_ALLOWED_ACE) };
        if ace.Mask != LOCK_MASK {
            return false;
        }
        let sid = PSID(std::ptr::addr_of!(ace.SidStart) as *mut _);
        // SAFETY: SidStart begins a SID that fits within AceSize.
        unsafe { EqualSid(sid, everyone.psid()) }.is_ok()
    }
}
