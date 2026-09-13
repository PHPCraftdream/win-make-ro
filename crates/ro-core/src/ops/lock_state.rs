use std::path::Path;

use crate::types::{Error, LockState, Result};
use crate::win::acl::{Dacl, state_of};
use crate::win::{Sid, wide_path};

pub fn lock_state(path: &Path) -> Result<LockState> {
    let wide = wide_path(path).map_err(|e| Error::os(path, e))?;
    let dacl = Dacl::read(path, &wide)?;
    Ok(state_of(&dacl, &Sid::everyone()))
}
