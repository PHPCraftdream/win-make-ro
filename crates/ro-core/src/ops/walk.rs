use std::path::Path;

use crate::types::{Error, Report};

/// Depth-first visit of the subtree below `root` (root excluded). Symlinks
/// and junctions are neither descended nor visited; they count as skipped.
pub fn walk(root: &Path, report: &mut Report, mut visit: impl FnMut(&Path, &mut Report)) {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                report.errors.push(Error::os(&dir, e));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    report.errors.push(Error::os(&dir, e));
                    continue;
                }
            };
            let path = entry.path();
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(e) => {
                    report.errors.push(Error::os(&path, e));
                    continue;
                }
            };
            if ft.is_symlink() {
                report.skipped += 1;
                continue;
            }
            visit(&path, report);
            if ft.is_dir() {
                stack.push(path);
            }
        }
    }
}
