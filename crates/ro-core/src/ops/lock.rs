use std::path::Path;

use crate::types::{Error, ErrorKind, Result};
use crate::win::acl::{AclBuilder, Dacl, ace_size, aces, write_dacl};
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
    // A NULL DACL grants everyone everything; an ACL holding only our deny
    // would instead refuse everything, so materialise the implied allow.
    let was_null = dacl.acl.is_null();
    let extra = ace_size(&everyone) * if was_null { 2 } else { 1 };
    let mut b = AclBuilder::new(dacl.acl, extra);
    let io = |e| Error::os(path, e);
    // Canonical order: explicit denies first, so the lock goes to the front.
    b.push_lock(&everyone, meta.is_dir()).map_err(io)?;
    if was_null {
        b.push_allow_all(&everyone).map_err(io)?;
    }
    for a in old.iter() {
        b.push(*a).map_err(io)?;
    }
    // Files: the attribute blocks delete/rename via parent FILE_DELETE_CHILD.
    // Set it first; once the DACL is in place nobody may write attributes.
    let mut attr_set = false;
    if !meta.is_dir() {
        match set_readonly_attr(path, true) {
            Ok(changed) => attr_set = changed,
            // A locked parent denies WRITE_ATTRIBUTES, so the attribute cannot
            // be set — but it also denies the FILE_DELETE_CHILD the attribute
            // guards against, so the lock is still complete without it.
            Err(_) if old.iter().any(|a| a.inherited() && a.is_lock(&everyone)) => {}
            Err(e) => return Err(Error::os(path, e)),
        }
    }
    if let Err(e) = write_dacl(path, &wide, b.acl()) {
        if attr_set {
            let _ = set_readonly_attr(path, false);
        }
        return Err(e);
    }
    Ok(true)
}
