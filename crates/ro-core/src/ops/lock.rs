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
    // A NULL DACL is not an empty list of permissions: it switches the access
    // check off entirely, so even a token holding `Everyone` as deny-only is
    // granted everything. No ACL can reproduce that, and no later unlock could
    // tell a reconstructed one from a deliberate `Everyone: FullControl`, so
    // the item is refused rather than silently converted.
    if dacl.acl.is_null() {
        return Err(Error::new(path, ErrorKind::NullDacl));
    }
    let old = aces(dacl.acl);
    if old.iter().any(|a| !a.inherited() && a.is_lock(&everyone)) {
        return Ok(false);
    }
    let io = |e| Error::os(path, e);
    let mut b = AclBuilder::new(dacl.acl, ace_size(&everyone)).map_err(io)?;
    // Canonical order: explicit denies first, so the lock goes to the front.
    b.push_lock(&everyone, meta.is_dir()).map_err(io)?;
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
