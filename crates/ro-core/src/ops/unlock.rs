use std::path::Path;

use crate::types::{Error, ErrorKind, Result};
use crate::win::acl::{AceRef, AclBuilder, Dacl, aces, write_dacl};
use crate::win::{Sid, set_readonly_attr, wide_path};

/// Removes the explicit lock ACE (and the READONLY attribute) from one item.
/// `Ok(false)` when there was none; `LockedByParent` while a parent's lock is
/// still in force, since removing the item's own ACE could not make it
/// writable and would only desynchronise the ACE from the attribute.
pub fn unlock(path: &Path) -> Result<bool> {
    let wide = wide_path(path).map_err(|e| Error::os(path, e))?;
    let meta = std::fs::symlink_metadata(path).map_err(|e| Error::os(path, e))?;
    if meta.file_type().is_symlink() {
        return Err(Error::new(path, ErrorKind::ReparsePoint));
    }
    let everyone = Sid::everyone();
    let dacl = Dacl::read(path, &wide)?;
    let all = aces(dacl.acl);
    if all.iter().any(|a| a.inherited() && a.is_lock(&everyone)) {
        return Err(Error::new(path, ErrorKind::LockedByParent));
    }
    if !all.iter().any(|a| a.is_lock(&everyone)) {
        return Ok(false);
    }
    let keep: Vec<_> = all.iter().copied().filter(|a| !a.is_lock(&everyone)).collect();
    write_kept(path, &wide, &dacl, &keep)?;
    if !meta.is_dir() {
        if let Err(e) = set_readonly_attr(path, false) {
            // Put the lock back rather than leave the item half-unlocked: an
            // orphaned READONLY attribute reads as "unlocked" yet refuses
            // writes.
            let _ = write_kept(path, &wide, &dacl, &all);
            return Err(Error::os(path, e));
        }
    }
    Ok(true)
}

fn write_kept(path: &Path, wide: &[u16], dacl: &Dacl, keep: &[AceRef]) -> Result<()> {
    let mut b = AclBuilder::new(dacl.acl, 0);
    for a in keep {
        b.push(*a).map_err(|e| Error::os(path, e))?;
    }
    write_dacl(path, wide, b.acl())
}
