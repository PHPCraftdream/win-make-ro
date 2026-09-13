use std::path::Path;

use super::{lock_state, unlock, walk};
use crate::types::{ErrorKind, LockState, Report};

/// Unlocks `root` and every explicit lock below it.
pub fn unlock_tree(root: &Path) -> Report {
    let mut report = Report::default();
    match unlock(root) {
        Err(e) if matches!(e.kind, ErrorKind::LockedByParent) => {
            report.errors.push(e);
            return report;
        }
        r => report.record(r),
    }
    if root.is_dir() {
        walk(root, &mut report, |p, rep| match lock_state(p) {
            Ok(LockState::Explicit) => rep.record(unlock(p)),
            Ok(_) => {}
            Err(e) => rep.errors.push(e),
        });
    }
    report
}
