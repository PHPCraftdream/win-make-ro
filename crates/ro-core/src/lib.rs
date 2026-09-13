//! NTFS "read only" lock: an explicit deny ACE for `Everyone` with a fixed
//! access mask. The exact mask identifies the lock as ours; no extra marker.
//!
//! ASSUMES: windows 0.62.2 (resolved in Cargo.lock).

#![cfg(windows)]

mod ops;
mod types;
mod win;

pub use ops::{lock, lock_state, lock_tree, unlock, unlock_tree};
pub use types::{Error, ErrorKind, LOCK_MASK, LockState, Report, Result};
pub use win::wide_path;
