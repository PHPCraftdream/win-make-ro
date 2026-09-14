use crate::types::LockState;
use crate::win::Sid;
use crate::win::acl::{Dacl, aces};

/// An inherited lock outranks an explicit one: while a parent's lock applies,
/// `unlock` refuses, so reporting `Explicit` would promise a removal that
/// cannot happen.
pub fn state_of(dacl: &Dacl, everyone: &Sid) -> LockState {
    let mut explicit = false;
    for ace in aces(dacl.acl) {
        if ace.is_lock(everyone) {
            if ace.inherited() {
                return LockState::Inherited;
            }
            explicit = true;
        }
    }
    if explicit { LockState::Explicit } else { LockState::Unlocked }
}
