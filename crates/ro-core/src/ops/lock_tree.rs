use std::path::Path;

use super::{lock, lock_needed, walk};
use crate::types::{ErrorKind, Report};

/// Locks `root`; for a directory also ensures every descendant is locked,
/// explicitly wherever the inherited lock alone would not hold.
pub fn lock_tree(root: &Path) -> Report {
    let mut report = Report::default();
    let outcome = lock(root);
    // A reparse point root is refused, and must not be walked either: the walk
    // would follow the link and rewrite ACLs inside its target.
    let reparse = matches!(&outcome, Err(e) if matches!(e.kind, ErrorKind::ReparsePoint));
    report.record(outcome);
    if reparse {
        report.skipped += 1;
        return report;
    }
    if root.is_dir() {
        walk(root, &mut report, |p, rep| match lock_needed(p) {
            Ok(true) => rep.record(lock(p)),
            Ok(false) => {}
            Err(e) => rep.errors.push(e),
        });
    }
    report
}
