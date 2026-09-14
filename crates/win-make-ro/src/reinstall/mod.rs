//! Replacing an installation that Explorer is currently using.
//!
//! Windows will not let a mapped image be overwritten or deleted, and Explorer
//! maps `ro_shellext.dll` the first time a context menu is opened and keeps it
//! until it exits. Renaming is allowed, so a new installation takes the name
//! and the old file waits under another one until Explorer has been restarted.

mod binaries;
mod explorer;
mod outcome;
mod run;
mod swap;
mod sweep;

pub use outcome::Restart;
pub use run::reinstall;
