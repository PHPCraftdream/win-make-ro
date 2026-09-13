use std::path::Path;

use super::{lock_state, unlock, walk};
use crate::types::{ErrorKind, LockState, Report};

/// Unlocks `root` and every explicit lock below it.
pub fn unlock_tree(root: &Path) -> Report {
    let mut report = Report::default();
    let outcome = unlock(root);
    match &outcome {
        // Nothing below a parent-locked root may be touched either.
        Err(e) if matches!(e.kind, ErrorKind::LockedByParent) => {
            report.errors.push(outcome.unwrap_err());
            return report;
        }
        // Same as lock_tree: never walk through a reparse point.
        Err(e) if matches!(e.kind, ErrorKind::ReparsePoint) => {
            report.record(outcome);
            report.skipped += 1;
            return report;
        }
        _ => report.record(outcome),
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
