use windows::Win32::Security::{ACCESS_ALLOWED_ACE, ACE_HEADER, EqualSid, INHERITED_ACE, PSID};
use windows::Win32::Storage::FileSystem::FILE_ALL_ACCESS;

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

    pub fn inherited(&self) -> bool {
        u32::from(self.header().AceFlags) & INHERITED_ACE.0 != 0
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

    /// True for a lock ACE (explicit or inherited): deny, Everyone, exact mask.
    pub fn is_lock(&self, everyone: &Sid) -> bool {
        self.header().AceType == DENY_TYPE && self.mask() == LOCK_MASK && self.is_for(everyone)
    }

    /// True for the allow ACE that materialises NULL DACL semantics.
    pub fn is_allow_all(&self, everyone: &Sid) -> bool {
        self.is_allow()
            && !self.inherited()
            && self.mask() == FILE_ALL_ACCESS.0
            && self.is_for(everyone)
    }
}
