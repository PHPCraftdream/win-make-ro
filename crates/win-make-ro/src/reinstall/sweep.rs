use std::fs;
use std::path::{Path, PathBuf};

use super::binaries::BINARIES;

/// Removes the superseded binaries [`super::swap`] renamed aside, and returns
/// the ones that would not go.
///
/// A copy still mapped into a running Explorer refuses to be deleted, which is
/// why this runs again after Explorer has been restarted — and why the next
/// reinstall sweeps before it does anything else, in case a previous one never
/// got that far.
pub fn sweep(dir: &Path) -> Vec<PathBuf> {
    let mut left = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return left;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if superseded(name) && fs::remove_file(&path).is_err() {
            left.push(path);
        }
    }
    left
}

/// `<binary>.<stamp>.old` — the shape and nothing looser: this deletes files.
fn superseded(name: &str) -> bool {
    name.ends_with(".old")
        && BINARIES.iter().any(|b| name.strip_prefix(b).is_some_and(|r| r.starts_with('.')))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_binaries_we_renamed_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mine = ["win-make-ro.exe.123-456.old", "ro_shellext.dll.7-8.old"];
        let theirs = ["ro_shellext.dll", "win-make-ro.exe", "notes.old", "something.dll.old"];
        for name in mine.iter().chain(theirs.iter()) {
            fs::write(dir.path().join(name), b"x").unwrap();
        }

        assert!(sweep(dir.path()).is_empty());
        for name in mine {
            assert!(!dir.path().join(name).exists(), "{name} survived");
        }
        for name in theirs {
            assert!(dir.path().join(name).exists(), "{name} was not ours to delete");
        }
    }

    /// A copy Explorer still has mapped cannot be deleted; saying so is the
    /// point, since the caller reports it rather than pretending it is gone.
    #[test]
    fn a_file_that_will_not_go_is_reported() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let stuck = dir.path().join("ro_shellext.dll.1-2.old");
        fs::write(&stuck, b"x").unwrap();
        let _locked =
            OpenOptions::new().read(true).share_mode(0).open(&stuck).expect("lock the copy");

        assert_eq!(sweep(dir.path()), vec![stuck.clone()]);
        assert!(stuck.exists());
    }

    #[test]
    fn a_directory_that_is_not_there_is_not_an_error() {
        assert!(sweep(Path::new(r"C:\no\such\directory\here")).is_empty());
    }
}
