use std::path::Path;

use super::{lock, lock_state, walk};
use crate::types::{LockState, Report};

/// Locks `root`; for a directory also ensures every descendant is locked
/// (explicitly where inheritance is blocked).
pub fn lock_tree(root: &Path) -> Report {
    let mut report = Report::default();
    report.record(lock(root));
    if root.is_dir() {
        walk(root, &mut report, |p, rep| match lock_state(p) {
            Ok(LockState::Unlocked) => rep.record(lock(p)),
            Ok(_) => {}
            Err(e) => rep.errors.push(e),
        });
    }
    report
}
