use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::binaries::Binaries;

/// Puts the binaries from `from` in place of the ones in `to`, and returns the
/// superseded copies it made.
///
/// A file that is mapped into a running process cannot be overwritten or
/// deleted, and Explorer keeps `ro_shellext.dll` mapped for as long as it runs.
/// It can be renamed, though: that touches only the directory entry, so the
/// running Explorer keeps the image it already has while the name is freed for
/// the new file. The returned copies are removed once Explorer has gone.
///
/// Both new files are written under temporary names before any existing one is
/// disturbed, so the part that can fail on content — copying — is over before
/// the installation stops being whole. What is left after that is renaming
/// within a single directory.
///
/// A failure undoes what it can and says what it could not: the rollback moves
/// files too, and promising it cannot fail would be a promise about the file
/// system rather than about this code.
pub fn swap(from: &Path, to: &Path) -> io::Result<Vec<PathBuf>> {
    let stamp = stamp();
    let mut staged: Vec<(PathBuf, PathBuf)> = Vec::new(); // (staged copy, final name)
    for name in Binaries::NAMES {
        let src = from.join(name);
        if !src.is_file() {
            remove_all(staged.iter().map(|(new, _)| new));
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} is not there to install", src.display()),
            ));
        }
        let new = to.join(Binaries::staged(name, &stamp));
        if let Err(e) = fs::copy(&src, &new) {
            remove_all(staged.iter().map(|(new, _)| new).chain(std::iter::once(&new)));
            return Err(e);
        }
        staged.push((new, to.join(name)));
    }

    // Nothing below copies content; these are directory-entry moves.
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new(); // (final name, what it displaced)
    for (index, (new, final_name)) in staged.iter().enumerate() {
        let left = || staged.iter().skip(index).map(|(new, _)| new);
        let aside = if final_name.exists() {
            let a = to.join(Binaries::superseded(Binaries::NAMES[index], &stamp));
            if let Err(e) = fs::rename(final_name, &a) {
                return Err(undo(done, left(), e));
            }
            Some(a)
        } else {
            None
        };
        if let Err(e) = fs::rename(new, final_name) {
            if let Some(a) = &aside {
                let _ = fs::rename(a, final_name);
            }
            return Err(undo(done, left(), e));
        }
        if let Some(a) = aside {
            done.push((final_name.clone(), a));
        }
    }
    Ok(done.into_iter().map(|(_, aside)| aside).collect())
}

fn stamp() -> String {
    let nanos =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

fn remove_all<'a>(paths: impl Iterator<Item = &'a PathBuf>) {
    for p in paths {
        let _ = fs::remove_file(p);
    }
}

/// Puts back what was already exchanged and clears away what was staged,
/// folding anything that resisted into the error the caller sees.
fn undo<'a>(
    done: Vec<(PathBuf, PathBuf)>,
    staged: impl Iterator<Item = &'a PathBuf>,
    cause: io::Error,
) -> io::Error {
    remove_all(staged);
    let mut stuck = Vec::new();
    for (final_name, aside) in done {
        if fs::remove_file(&final_name).is_err() || fs::rename(&aside, &final_name).is_err() {
            stuck.push(final_name.display().to_string());
        }
    }
    if stuck.is_empty() {
        return cause;
    }
    io::Error::other(format!("{cause}; and these were left replaced: {}", stuck.join(", ")))
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
        for name in Binaries::NAMES {
            fs::write(from.join(name), b"new").unwrap();
            if let Some(bytes) = old {
                fs::write(to.join(name), bytes).unwrap();
            }
        }
        (dir, from, to)
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn untouched() -> Vec<String> {
        let mut v: Vec<String> = Binaries::NAMES.iter().map(|s| (*s).to_string()).collect();
        v.sort();
        v
    }

    #[test]
    fn the_old_binaries_are_renamed_aside_not_overwritten() {
        let (_d, from, to) = dirs(Some("old"));
        let superseded = swap(&from, &to).unwrap();
        for name in Binaries::NAMES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"new", "{name} was not replaced");
        }
        assert_eq!(superseded.len(), 2, "{superseded:?}");
        for p in &superseded {
            assert_eq!(fs::read(p).unwrap(), b"old", "{} lost its contents", p.display());
        }
        // Nothing staged survives the exchange.
        assert!(!listing(&to).iter().any(|n| n.ends_with(".new")), "{:?}", listing(&to));
    }

    /// A first install has nothing to rename and must still work.
    #[test]
    fn an_empty_destination_just_receives_the_binaries() {
        let (_d, from, to) = dirs(None);
        assert!(swap(&from, &to).unwrap().is_empty(), "nothing was there to set aside");
        for name in Binaries::NAMES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"new");
        }
        assert_eq!(listing(&to), untouched());
    }

    #[test]
    fn a_missing_source_stops_before_anything_moves() {
        let (_d, from, to) = dirs(Some("old"));
        fs::remove_file(from.join(Binaries::NAMES[1])).unwrap();
        assert!(swap(&from, &to).is_err());
        for name in Binaries::NAMES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"old", "{name} was touched");
        }
        assert_eq!(listing(&to), untouched(), "a staged copy was left behind");
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
            .open(to.join(Binaries::NAMES[1]))
            .expect("lock the destination");
        let outcome = swap(&from, &to);
        // Released before reading anything back: the lock shuts this test out too.
        drop(locked);

        assert!(outcome.is_err(), "the locked file was replaced anyway");
        for name in Binaries::NAMES {
            assert_eq!(fs::read(to.join(name)).unwrap(), b"old", "{name} was left replaced");
        }
        assert_eq!(listing(&to), untouched(), "staged or superseded copies were left behind");
    }
}
