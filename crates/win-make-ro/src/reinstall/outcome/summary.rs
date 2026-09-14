use std::path::PathBuf;

use super::Restart;

/// What a `reinstall` did, once the installation part of it succeeded.
///
/// A failed restart does not undo the registration, so the two are reported
/// side by side rather than collapsed into one error.
#[derive(Debug)]
pub struct Summary {
    /// Where the binaries are registered from now.
    pub to: PathBuf,
    pub restart: Restart,
    /// Superseded copies that would not be deleted, left for the next run.
    pub left_behind: Vec<PathBuf>,
}
