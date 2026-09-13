use std::path::Path;

use crate::types::{Error, ErrorKind, LockState, Result};
use crate::win::acl::{AclBuilder, Dacl, aces, state_of, write_dacl, write_null_dacl};
use crate::win::{Sid, set_readonly_attr, wide_path};

/// Removes the explicit lock ACE (and the READONLY attribute) from one item.
/// `Ok(false)` when there was none; `LockedByParent` if only an inherited lock
/// exists.
pub fn unlock(path: &Path) -> Result<bool> {
    let wide = wide_path(path).map_err(|e| Error::os(path, e))?;
    let meta = std::fs::symlink_metadata(path).map_err(|e| Error::os(path, e))?;
    if meta.file_type().is_symlink() {
        return Err(Error::new(path, ErrorKind::ReparsePoint));
    }
    let everyone = Sid::everyone();
    let dacl = Dacl::read(path, &wide)?;
    match state_of(&dacl, &everyone) {
        LockState::Unlocked => return Ok(false),
        LockState::Inherited => return Err(Error::new(path, ErrorKind::LockedByParent)),
        LockState::Explicit => {}
    }
    let keep: Vec<_> =
        aces(dacl.acl).into_iter().filter(|a| a.inherited() || !a.is_lock(&everyone)).collect();
    // Exactly the allow-everything ACE `lock` writes for a NULL DACL: restore
    // the NULL DACL rather than leave a weaker explicit equivalent behind.
    if keep.len() == 1 && keep[0].is_allow_all(&everyone) {
        write_null_dacl(path, &wide)?;
    } else {
        let mut b = AclBuilder::new(dacl.acl, 0);
        for a in keep {
            b.push(a).map_err(|e| Error::os(path, e))?;
        }
        write_dacl(path, &wide, b.acl())?;
    }
    if !meta.is_dir() {
        set_readonly_attr(path, false).map_err(|e| Error::os(path, e))?;
    }
    Ok(true)
}
