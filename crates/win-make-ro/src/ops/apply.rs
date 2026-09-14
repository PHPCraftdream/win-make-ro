use std::path::{Path, PathBuf};

use ro_core::{ErrorKind, Report, lock_tree, unlock_tree};

use crate::cli::Command;

/// Runs the recursive operation over every path, merging the reports.
///
/// Unlocking a child while its parent is still locked is refused, so
/// `unlock child parent` used to report an error that `unlock parent child` did
/// not, for the same end state. Sorting the absolute paths settles the common
/// case, since a parent is a prefix of its children, and duplicates collapse on
/// the way. It settles no more than that: Windows spells the same item several
/// ways — a different case, a `\\?\` prefix, a short name — and none of those
/// meet under string order. A target that failed for no reason other than a
/// parent's lock is therefore tried again as long as some other target got
/// through, which may well have been the parent it was waiting for.
pub fn apply(command: Command, paths: &[PathBuf]) -> Report {
    let mut total = Report::default();
    let mut pending = ordered(paths);
    loop {
        let mut blocked = Vec::new();
        let mut progressed = false;
        for p in pending {
            let r = one(command, &p);
            if waits_for_a_parent(&r) {
                blocked.push((p, r));
            } else {
                progressed = true;
                merge(&mut total, r);
            }
        }
        // Every pass that gets anywhere leaves strictly fewer targets behind,
        // so this ends; one that gets nowhere keeps the refusals it collected.
        if blocked.is_empty() || !progressed {
            for (_, r) in blocked {
                merge(&mut total, r);
            }
            return total;
        }
        pending = blocked.into_iter().map(|(p, _)| p).collect();
    }
}

fn one(command: Command, path: &Path) -> Report {
    match command {
        Command::Lock => lock_tree(path),
        Command::Unlock => unlock_tree(path),
        _ => unreachable!("apply is only called for lock/unlock"),
    }
}

/// Whether the target did nothing at all and complained only about a parent's
/// lock — the one outcome another target can still turn into a success.
fn waits_for_a_parent(r: &Report) -> bool {
    r.changed == 0
        && r.skipped == 0
        && !r.errors.is_empty()
        && r.errors.iter().all(|e| matches!(e.kind, ErrorKind::LockedByParent))
}

fn merge(total: &mut Report, r: Report) {
    total.changed += r.changed;
    total.skipped += r.skipped;
    total.errors.extend(r.errors);
}

fn ordered(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = paths
        .iter()
        // A path that cannot be made absolute is kept as given, so the error
        // surfaces from the operation rather than from the ordering.
        .map(|p| std::path::absolute(p).unwrap_or_else(|_| p.clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::ordered;
    use std::path::PathBuf;

    fn order(paths: &[&str]) -> Vec<String> {
        ordered(&paths.iter().map(PathBuf::from).collect::<Vec<_>>())
            .iter()
            .map(|p| p.display().to_string())
            .collect()
    }

    #[test]
    fn a_parent_comes_before_its_child() {
        let child = r"C:\root\sub\file.txt";
        let parent = r"C:\root";
        assert_eq!(order(&[child, parent]), vec![parent.to_string(), child.to_string()]);
        assert_eq!(order(&[parent, child]), vec![parent.to_string(), child.to_string()]);
    }

    #[test]
    fn duplicates_collapse() {
        assert_eq!(order(&[r"C:\a", r"C:\a"]), vec![r"C:\a".to_string()]);
    }

    #[test]
    fn unrelated_paths_are_all_kept() {
        assert_eq!(order(&[r"C:\b", r"C:\a"]).len(), 2);
    }
}
