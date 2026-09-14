mod ace_ref;
mod aces;
mod builder;
mod dacl;
mod state;
mod write;

pub use ace_ref::AceRef;
pub use aces::aces;
pub use builder::{AclBuilder, ace_size};
pub use dacl::Dacl;
pub use state::{lock_shadowed, state_of};
pub use write::write_dacl;

/// `ACCESS_DENIED_ACE_TYPE` from winnt.h: the plain deny the lock is made of.
pub const DENY_TYPE: u8 = 1;

/// Every ACE type in winnt.h that refuses access: plain, object, callback and
/// callback-object. Anything else found in a DACL either grants access or is
/// unknown to us, and neither can be assumed harmless.
pub const DENY_TYPES: [u8; 4] = [
    DENY_TYPE, // ACCESS_DENIED_ACE_TYPE
    0x06,      // ACCESS_DENIED_OBJECT_ACE_TYPE
    0x0A,      // ACCESS_DENIED_CALLBACK_ACE_TYPE
    0x0C,      // ACCESS_DENIED_CALLBACK_OBJECT_ACE_TYPE
];
