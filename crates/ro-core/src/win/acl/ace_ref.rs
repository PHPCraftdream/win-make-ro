use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, EqualSid, INHERIT_ONLY_ACE, INHERITED_ACE, PSID,
};

use super::{ALLOW_TYPE, DENY_TYPE};
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

    pub fn flags(&self) -> u32 {
        u32::from(self.header().AceFlags)
    }

    pub fn inherited(&self) -> bool {
        self.flags() & INHERITED_ACE.0 != 0
    }

    /// An INHERIT_ONLY ACE is propagated to children but does not apply to the
    /// object holding it. See
    /// <https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-inheritance-rules>.
    pub fn inherit_only(&self) -> bool {
        self.flags() & INHERIT_ONLY_ACE.0 != 0
    }

    pub fn is_allow(&self) -> bool {
        self.header().AceType == ALLOW_TYPE
    }

    /// Access mask. Allow and deny ACEs share `ACCESS_ALLOWED_ACE`'s layout.
    pub fn mask(&self) -> u32 {
        // SAFETY: both ACE types are at least ACCESS_ALLOWED_ACE-sized.
        unsafe { (*(self.ptr as *const ACCESS_ALLOWED_ACE)).Mask }
    }

    fn is_for(&self, sid: &Sid) -> bool {
        // SAFETY: SidStart begins a SID that fits within AceSize.
        unsafe {
            let ace = &*(self.ptr as *const ACCESS_ALLOWED_ACE);
            EqualSid(PSID(std::ptr::addr_of!(ace.SidStart) as *mut _), sid.psid()).is_ok()
        }
    }

    /// True for a lock ACE (explicit or inherited) that applies to this very
    /// object: deny, Everyone, exact mask, not inherit-only.
    pub fn is_lock(&self, everyone: &Sid) -> bool {
        self.header().AceType == DENY_TYPE
            && !self.inherit_only()
            && self.mask() == LOCK_MASK
            && self.is_for(everyone)
    }
}
