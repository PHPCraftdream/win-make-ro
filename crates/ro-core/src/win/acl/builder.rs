use std::io;

use windows::Win32::Security::{
    ACE_HEADER, ACE_REVISION, ACL, ACL_REVISION, AddAce, CONTAINER_INHERIT_ACE, InitializeAcl,
    OBJECT_INHERIT_ACE,
};

use super::{AceRef, DENY_TYPE, aces};
use crate::types::LOCK_MASK;
use crate::win::Sid;

const MAXDWORD: u32 = u32::MAX;

/// The most an ACL can hold: `ACL::AclSize` is a `WORD`, and the buffer is
/// allocated in whole 4-byte words, so the usable maximum rounds down to 65 532.
/// See <https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-acl>.
const MAX_ACL_BYTES: usize = (u16::MAX as usize) & !3;

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
    /// Capacity = the bytes the old ACEs actually occupy + `extra_bytes`.
    ///
    /// The old `AclSize` counts free space in the buffer Windows handed us as
    /// well, so it is not what the copy needs. How large the source DACL is
    /// belongs to the file system, not to us: an ACL past the limit is an
    /// ordinary refusal, not a broken invariant.
    pub fn new(old: *const ACL, extra_bytes: usize) -> io::Result<Self> {
        let (in_use, revision) = if old.is_null() {
            (std::mem::size_of::<ACL>(), ACL_REVISION)
        } else {
            // SAFETY: valid ACL.
            let a = unsafe { &*old };
            let used: usize =
                aces(old).iter().map(|e| usize::from(e.header().AceSize)).sum::<usize>()
                    + std::mem::size_of::<ACL>();
            (used, ACE_REVISION(u32::from(a.AclRevision)))
        };
        let needed = in_use.saturating_add(extra_bytes);
        if needed > MAX_ACL_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the access control list would grow past the 64 KB Windows allows",
            ));
        }
        let words = needed.div_ceil(4);
        let mut buf = vec![0u32; words];
        // SAFETY: buffer is 4-byte aligned and `words*4` bytes long.
        unsafe { InitializeAcl(buf.as_mut_ptr() as *mut ACL, (words * 4) as u32, revision) }
            .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xFFFF))?;
        Ok(Self { buf, revision })
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
}
