use std::io;

use windows::Win32::Security::{
    ACE_HEADER, ACE_REVISION, ACL, ACL_REVISION, AddAce, CONTAINER_INHERIT_ACE, InitializeAcl,
    OBJECT_INHERIT_ACE,
};
use windows::Win32::Storage::FileSystem::FILE_ALL_ACCESS;

use super::{ALLOW_TYPE, AceRef, DENY_TYPE};
use crate::types::LOCK_MASK;
use crate::win::Sid;

const MAXDWORD: u32 = u32::MAX;

/// Bytes an ACE for `sid` occupies: header + mask + the SID itself.
pub fn ace_size(sid: &Sid) -> usize {
    std::mem::size_of::<ACE_HEADER>() + 4 + sid.len() as usize
}

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

    fn push_ace(&mut self, kind: u8, mask: u32, sid: &Sid, flags: u8) -> io::Result<()> {
        let sid_len = sid.len() as usize;
        let size = ace_size(sid);
        let mut bytes = vec![0u8; size];
        let header = ACE_HEADER { AceType: kind, AceFlags: flags, AceSize: size as u16 };
        // SAFETY: plain-old-data copies into a buffer of exactly `size` bytes.
        unsafe {
            std::ptr::write_unaligned(bytes.as_mut_ptr() as *mut ACE_HEADER, header);
            std::ptr::write_unaligned(bytes.as_mut_ptr().add(4) as *mut u32, mask);
            std::ptr::copy_nonoverlapping(
                sid.psid().0 as *const u8,
                bytes.as_mut_ptr().add(8),
                sid_len,
            );
        }
        self.push_raw(bytes.as_ptr() as *const _, size as u32)
    }

    /// Appends the lock ACE; `inheritable` = OI|CI for directories.
    pub fn push_lock(&mut self, everyone: &Sid, inheritable: bool) -> io::Result<()> {
        let flags = if inheritable { OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0 } else { 0 };
        self.push_ace(DENY_TYPE, LOCK_MASK, everyone, flags as u8)
    }

    /// Appends an allow-everything ACE for `Everyone`, the explicit equivalent
    /// of a NULL DACL. Never inheritable: a NULL DACL propagates nothing.
    pub fn push_allow_all(&mut self, everyone: &Sid) -> io::Result<()> {
        self.push_ace(ALLOW_TYPE, FILE_ALL_ACCESS.0, everyone, 0)
    }
}
