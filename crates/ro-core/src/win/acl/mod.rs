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

/// `ACCESS_ALLOWED_ACE_TYPE` from winnt.h.
pub const ALLOW_TYPE: u8 = 0;

/// `ACCESS_DENIED_ACE_TYPE` from winnt.h.
pub const DENY_TYPE: u8 = 1;
