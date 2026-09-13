use super::{Dacl, aces};
use crate::types::LockState;
use crate::win::Sid;

pub fn state_of(dacl: &Dacl, everyone: &Sid) -> LockState {
    let mut inherited = false;
    for ace in aces(dacl.acl) {
        if ace.is_lock(everyone) {
            if ace.inherited() {
                inherited = true;
            } else {
                return LockState::Explicit;
            }
        }
    }
    if inherited { LockState::Inherited } else { LockState::Unlocked }
}
