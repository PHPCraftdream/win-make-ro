use std::path::Path;

use crate::types::{Error, ErrorKind, Result};
use crate::win::acl::{AclBuilder, Dacl, aces, write_dacl};
use crate::win::{Sid, set_readonly_attr, wide_path};

/// Adds the lock ACE to one item (plus the READONLY attribute on files).
/// Idempotent. Directories get an inheritable ACE, which Windows propagates
/// to the whole subtree in this call.
pub fn lock(path: &Path) -> Result<bool> {
    let wide = wide_path(path).map_err(|e| Error::os(path, e))?;
    let meta = std::fs::symlink_metadata(path).map_err(|e| Error::os(path, e))?;
    if meta.file_type().is_symlink() {
        return Err(Error::new(path, ErrorKind::ReparsePoint));
    }
    let everyone = Sid::everyone();
    let dacl = Dacl::read(path, &wide)?;
    let old = aces(dacl.acl);
    if old.iter().any(|a| !a.inherited() && a.is_lock(&everyone)) {
        return Ok(false);
    }
    let mut b = AclBuilder::new(dacl.acl, 8 + everyone.len() as usize);
    let io = |e| Error::os(path, e);
    // Canonical order: explicit denies first, so the lock goes to the front.
    b.push_lock(&everyone, meta.is_dir()).map_err(io)?;
    for a in old {
        b.push(a).map_err(io)?;
    }
    // Files: the attribute blocks delete/rename via parent FILE_DELETE_CHILD.
    // Set it first; once the DACL is in place nobody may write attributes.
    if !meta.is_dir() {
        set_readonly_attr(path, true).map_err(io)?;
    }
    if let Err(e) = write_dacl(path, &wide, b.acl()) {
        if !meta.is_dir() {
            let _ = set_readonly_attr(path, false);
        }
        return Err(e);
    }
    Ok(true)
}
