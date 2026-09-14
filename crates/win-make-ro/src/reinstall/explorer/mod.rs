//! Closing Explorer and starting it again.
//!
//! The request to close is queued with `PostMessageW` and cannot be withdrawn,
//! so the moment it is sent somebody owes the machine a shell. That debt is
//! kept in one process, [`close_and_start`], which has no deadline; everything
//! else only watches it.

mod close;
mod elevated;
mod restart;
mod shell;

pub use close::close_and_start;
pub use restart::restart;
