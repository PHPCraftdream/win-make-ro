use crate::types::LOCK_MASK;
use crate::win::Sid;
use crate::win::acl::{Dacl, aces};

/// True when an allow ACE granting any locked right sits *before* the lock ACE.
///
/// Windows walks a DACL in order and stops at the first ACE that resolves the
/// requested right, so such an allow defeats the lock. The canonical ordering
/// puts explicit allows ahead of inherited denies, which is exactly how an
/// inherited lock loses to an explicit allow on a child.
/// See <https://learn.microsoft.com/en-us/windows/win32/secauthz/order-of-aces-in-a-dacl>.
pub fn lock_shadowed(dacl: &Dacl, everyone: &Sid) -> bool {
    for ace in aces(dacl.acl) {
        if ace.is_lock(everyone) {
            return false;
        }
        if ace.grants() && ace.mask() & LOCK_MASK != 0 {
            return true;
        }
    }
    false
}
