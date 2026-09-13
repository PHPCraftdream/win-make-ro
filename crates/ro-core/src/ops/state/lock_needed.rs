use std::path::Path;

use crate::types::{Error, LockState, Result};
use crate::win::acl::{Dacl, lock_shadowed, state_of};
use crate::win::{Sid, wide_path};

/// Whether a descendant still needs a lock ACE of its own.
///
/// An inherited lock is enough only while nothing shadows it: an explicit allow
/// granting a locked right is evaluated first and would keep the item writable,
/// so such an item gets an explicit lock too.
pub fn lock_needed(path: &Path) -> Result<bool> {
    let wide = wide_path(path).map_err(|e| Error::os(path, e))?;
    let dacl = Dacl::read(path, &wide)?;
    let everyone = Sid::everyone();
    Ok(match state_of(&dacl, &everyone) {
        LockState::Explicit => false,
        LockState::Unlocked => true,
        LockState::Inherited => lock_shadowed(&dacl, &everyone),
    })
}
