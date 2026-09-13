use std::path::Path;

use crate::types::{Error, ErrorKind, LockState, Result};
use crate::win::acl::{AclBuilder, Dacl, aces, state_of, write_dacl};
use crate::win::{Sid, wide_path};

/// Removes the explicit lock ACE from one item. `Ok(false)` when there was
/// none; `LockedByParent` if only an inherited lock exists.
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
    let mut b = AclBuilder::new(dacl.acl, 0);
    for a in aces(dacl.acl).into_iter().filter(|a| a.inherited() || !a.is_lock(&everyone)) {
        b.push(a).map_err(|e| Error::os(path, e))?;
    }
    write_dacl(path, &wide, b.acl())?;
    Ok(true)
}
