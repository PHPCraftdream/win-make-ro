use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::binaries::BINARIES;

/// Puts the binaries from `from` in place of the ones in `to`.
///
/// A file that is mapped into a running process cannot be overwritten or
/// deleted, and Explorer keeps `ro_shellext.dll` mapped for as long as it runs.
/// It can be renamed, though: that touches only the directory entry, so the
/// running Explorer keeps the image it already has while the name is freed for
/// the new file. The superseded copies are left behind for [`super::sweep`] to
/// remove once Explorer has gone.
///
/// Either every binary is replaced or none is: a failure puts back what was
/// renamed and removes what was copied.
pub fn swap(from: &Path, to: &Path) -> io::Result<()> {
    for name in BINARIES {
        let src = from.join(name);
        if !src.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not there to install", src.display()),
            ));
        }
    }

    // (what was put in place, what it displaced) for everything done so far.
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    for name in BINARIES {
        let dst = to.join(name);
        let aside = if dst.exists() {
            let a = to.join(aside_name(name));
            if let Err(e) = fs::rename(&dst, &a) {
                undo(&done);
                return Err(e);
            }
            Some(a)
        } else {
            None
        };
        let copied = fs::copy(from.join(name), &dst);
        done.push((dst, aside));
        if let Err(e) = copied {
            undo(&done);
            return Err(e);
        }
    }
    Ok(())
}

/// A name nothing else will be using, in the same directory so the rename
/// stays on one volume and cannot turn into a copy.
fn aside_name(binary: &str) -> String {
    let stamp =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("{binary}.{}-{stamp}.old", std::process::id())
}

fn undo(done: &[(PathBuf, Option<PathBuf>)]) {
    for (dst, aside) in done {
        let _ = fs::remove_file(dst);
        if let Some(a) = aside {
            let _ = fs::rename(a, dst);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    use super::*;

    /// Two directories: `from` holding the new binaries, `to` the old ones.
    fn dirs(old: Option<&str>) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("new");
        let to = dir.path().join("installed");
        fs::create_dir_all(&from).unwrap();
        fs::create_dir_all(&to).unwrap();
        for name in BINARIES {
            fs::write(from.join(name), b"new").unwrap();
            if let Some(bytes) = old {
                fs::write(to.join(name), bytes).unwrap();
            }
        }
        (dir, from, to)
    }

    fn olds(dir: &Path) -> Vec<PathBuf> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "old"))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn the_old_binaries_are_renamed_aside_not_overwritten() {
        let (_d, from, to) = dirs(Some("old"));
        swap(&from, &to).unwrap();
        for name in BINARIES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"new", "{name} was not replaced");
        }
        let aside = olds(&to);
        assert_eq!(aside.len(), 2, "{aside:?}");
        for p in aside {
            assert_eq!(fs::read(&p).unwrap(), b"old", "{} lost its contents", p.display());
        }
    }

    /// A first install has nothing to rename and must still work.
    #[test]
    fn an_empty_destination_just_receives_the_binaries() {
        let (_d, from, to) = dirs(None);
        swap(&from, &to).unwrap();
        for name in BINARIES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"new");
        }
        assert!(olds(&to).is_empty(), "nothing was there to set aside");
    }

    #[test]
    fn a_missing_source_stops_before_anything_moves() {
        let (_d, from, to) = dirs(Some("old"));
        fs::remove_file(from.join(BINARIES[1])).unwrap();
        assert!(swap(&from, &to).is_err());
        for name in BINARIES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"old", "{name} was touched");
        }
        assert!(olds(&to).is_empty());
    }

    /// Failing halfway must not leave an installation made of one new binary
    /// and one old one — the DLL and the helper are only tested together.
    #[test]
    fn a_failure_on_the_second_binary_puts_the_first_one_back() {
        let (_d, from, to) = dirs(Some("old"));
        // Opened with no sharing at all, so nothing else may touch it.
        let locked = OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(to.join(BINARIES[1]))
            .expect("lock the destination");
        let outcome = swap(&from, &to);
        // Released before reading anything back: the lock shuts this test out too.
        drop(locked);

        assert!(outcome.is_err(), "the locked file was replaced anyway");
        for name in BINARIES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"old", "{name} was left replaced");
        }
        assert!(olds(&to).is_empty(), "a renamed copy was left behind: {:?}", olds(&to));
    }
}
