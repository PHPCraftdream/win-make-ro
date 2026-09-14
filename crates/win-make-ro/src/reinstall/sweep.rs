use std::fs;
use std::path::{Path, PathBuf};

use super::binaries::Binaries;

/// Removes the superseded copies [`super::swap`] renamed aside, and returns the
/// ones that would not go.
///
/// `made_here` is the list the swap in this very run produced — the only real
/// proof of ownership there is. `dir` is then scanned for copies an earlier run
/// never got to delete, which no list outlives; those are recognised by the
/// shape of the name, and [`Binaries::is_superseded`] keeps that shape narrow,
/// because this deletes files.
///
/// A copy still mapped into a running Explorer refuses to be deleted, which is
/// why this runs again after Explorer has been restarted.
pub fn sweep(dir: &Path, made_here: &[PathBuf]) -> Vec<PathBuf> {
    let mut left = Vec::new();
    for path in made_here {
        if path.exists() && fs::remove_file(path).is_err() {
            left.push(path.clone());
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return left;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if Binaries::is_superseded(name) && !left.contains(&path) && fs::remove_file(&path).is_err()
        {
            left.push(path);
        }
    }
    left
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, names: &[&str]) {
        for name in names {
            fs::write(dir.join(name), b"x").unwrap();
        }
    }

    #[test]
    fn only_the_copies_this_tool_made_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let ours = ["win-make-ro.exe.superseded-123-456.old", "ro_shellext.dll.superseded-7-8.old"];
        // Names a person writes: a hand-made backup must survive a reinstall.
        let theirs = [
            "ro_shellext.dll",
            "win-make-ro.exe",
            "ro_shellext.dll.old",
            "ro_shellext.dll.manual-backup.old",
            "ro_shellext.dll.superseded.old",
            "ro_shellext.dll.superseded-a-b.old",
            "notes.old",
            "something.dll.superseded-1-2.old",
        ];
        write(dir.path(), &ours);
        write(dir.path(), &theirs);

        assert!(sweep(dir.path(), &[]).is_empty());
        for name in ours {
            assert!(!dir.path().join(name).exists(), "{name} survived");
        }
        for name in theirs {
            assert!(dir.path().join(name).exists(), "{name} was not ours to delete");
        }
    }

    /// The list the swap produced is the one proof of ownership, so it goes
    /// whatever the name happens to look like.
    #[test]
    fn the_copies_named_by_the_caller_go_whatever_they_are_called() {
        let dir = tempfile::tempdir().unwrap();
        let odd = dir.path().join("ro_shellext.dll.something-else");
        fs::write(&odd, b"x").unwrap();
        assert!(sweep(dir.path(), std::slice::from_ref(&odd)).is_empty());
        assert!(!odd.exists());
    }

    /// A copy Explorer still has mapped cannot be deleted; saying so is the
    /// point, since the caller reports it rather than pretending it is gone.
    #[test]
    fn a_file_that_will_not_go_is_reported_once() {
        use std::fs::OpenOptions;
        use std::os::windows::fs::OpenOptionsExt;

        let dir = tempfile::tempdir().unwrap();
        let stuck = dir.path().join("ro_shellext.dll.superseded-1-2.old");
        fs::write(&stuck, b"x").unwrap();
        let _locked =
            OpenOptions::new().read(true).share_mode(0).open(&stuck).expect("lock the copy");

        // Named by the caller *and* matching the scan: reported exactly once.
        assert_eq!(sweep(dir.path(), std::slice::from_ref(&stuck)), vec![stuck.clone()]);
        assert!(stuck.exists());
    }

    #[test]
    fn a_directory_that_is_not_there_is_not_an_error() {
        assert!(sweep(Path::new(r"C:\no\such\directory\here"), &[]).is_empty());
    }
}
