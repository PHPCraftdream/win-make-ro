use std::path::PathBuf;

use ro_core::{Report, lock_tree, unlock_tree};

use crate::cli::Command;

/// Runs the recursive operation over every path, merging the reports.
pub fn apply(command: Command, paths: &[PathBuf]) -> Report {
    let mut total = Report::default();
    for p in paths {
        let r = match command {
            Command::Lock => lock_tree(p),
            Command::Unlock => unlock_tree(p),
            _ => unreachable!("apply is only called for lock/unlock"),
        };
        total.changed += r.changed;
        total.skipped += r.skipped;
        total.errors.extend(r.errors);
    }
    total
}
