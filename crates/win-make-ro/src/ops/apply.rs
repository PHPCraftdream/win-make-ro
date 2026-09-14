use std::path::PathBuf;

use ro_core::{Report, lock_tree, unlock_tree};

use crate::cli::Command;

/// Runs the recursive operation over every path, merging the reports.
///
/// The targets are put in order first. Unlocking a child while its parent is
/// still locked is refused, so `unlock child parent` used to report an error
/// that `unlock parent child` did not, for the same end state. Sorting the
/// absolute paths puts every ancestor ahead of its descendants — a parent is a
/// prefix of its children — and duplicates collapse on the way.
pub fn apply(command: Command, paths: &[PathBuf]) -> Report {
    let mut total = Report::default();
    for p in ordered(paths) {
        let r = match command {
            Command::Lock => lock_tree(&p),
            Command::Unlock => unlock_tree(&p),
            _ => unreachable!("apply is only called for lock/unlock"),
        };
        total.changed += r.changed;
        total.skipped += r.skipped;
        total.errors.extend(r.errors);
    }
    total
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
