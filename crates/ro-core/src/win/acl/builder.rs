use std::io;

use windows::Win32::Security::{
    ACE_HEADER, ACE_REVISION, ACL, ACL_REVISION, AddAce, CONTAINER_INHERIT_ACE, InitializeAcl,
    OBJECT_INHERIT_ACE,
};

use super::{AceRef, DENY_TYPE};
use crate::types::LOCK_MASK;
use crate::win::Sid;

const MAXDWORD: u32 = u32::MAX;

/// Assembles a new ACL in a 4-byte-aligned buffer, keeping the old revision.
pub struct AclBuilder {
    buf: Vec<u32>,
    revision: ACE_REVISION,
}

impl AclBuilder {
    /// Capacity = old ACL size + `extra_bytes`.
    pub fn new(old: *const ACL, extra_bytes: usize) -> Self {
        let (in_use, revision) = if old.is_null() {
            (std::mem::size_of::<ACL>(), ACL_REVISION)
        } else {
            // SAFETY: valid ACL.
            let a = unsafe { &*old };
            (usize::from(a.AclSize), ACE_REVISION(u32::from(a.AclRevision)))
        };
        let words = (in_use + extra_bytes).div_ceil(4);
        let mut buf = vec![0u32; words];
        // SAFETY: buffer is 4-byte aligned and `words*4` bytes long.
        unsafe { InitializeAcl(buf.as_mut_ptr() as *mut ACL, (words * 4) as u32, revision) }
            .expect("invariant: fresh buffer of valid size");
        Self { buf, revision }
    }

    pub fn acl(&mut self) -> *mut ACL {
        self.buf.as_mut_ptr() as *mut ACL
    }

    fn push_raw(&mut self, ace: *const core::ffi::c_void, size: u32) -> io::Result<()> {
        // SAFETY: ace points at `size` bytes of a well-formed ACE; capacity was pre-computed.
        unsafe { AddAce(self.acl(), self.revision, MAXDWORD, ace, size) }
            .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))
    }

    pub fn push(&mut self, ace: AceRef) -> io::Result<()> {
        self.push_raw(ace.ptr as *const _, u32::from(ace.header().AceSize))
    }

    /// Appends the lock ACE; `inheritable` = OI|CI for directories.
    pub fn push_lock(&mut self, everyone: &Sid, inheritable: bool) -> io::Result<()> {
        let sid_len = everyone.len() as usize;
        let size = std::mem::size_of::<ACE_HEADER>() + 4 + sid_len;
        let mut bytes = vec![0u8; size];
        let flags = if inheritable { OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0 } else { 0 };
        let header = ACE_HEADER { AceType: DENY_TYPE, AceFlags: flags as u8, AceSize: size as u16 };
        // SAFETY: plain-old-data copies into a buffer of exactly `size` bytes.
        unsafe {
            std::ptr::write_unaligned(bytes.as_mut_ptr() as *mut ACE_HEADER, header);
            std::ptr::write_unaligned(bytes.as_mut_ptr().add(4) as *mut u32, LOCK_MASK);
            std::ptr::copy_nonoverlapping(
                everyone.psid().0 as *const u8,
                bytes.as_mut_ptr().add(8),
                sid_len,
            );
        }
        self.push_raw(bytes.as_ptr() as *const _, size as u32)
    }
}
