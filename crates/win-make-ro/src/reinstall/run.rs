use std::path::{Path, PathBuf};

use super::binaries::BINARIES;
use super::elevated::elevated;
use super::explorer;
use super::swap::swap;
use super::sweep::sweep;

/// Installs the binaries sitting next to this executable over the registered
/// ones, then restarts Explorer.
///
/// Explorer keeps `ro_shellext.dll` mapped for as long as it runs, so a plain
/// copy over it is refused by Windows and an install that only re-registers
/// would leave the old code in place. The superseded files are renamed aside
/// instead — a rename touches the directory entry, not the mapping — and are
/// deleted once Explorer has closed.
///
/// Explorer is restarted only when something was registered before this ran.
/// A first install has nothing loaded to replace, and closing the desktop to
/// prove it would be rude — which is what lets an `npm install -g` call this
/// unconditionally and still leave a fresh machine alone.
///
/// Returns where the binaries went, whether Explorer was restarted, and any
/// superseded copy that still would not go, which the next run sweeps.
pub fn reinstall() -> Result<(PathBuf, bool, Vec<PathBuf>), String> {
    // Explorer would inherit this process's token, and an elevated desktop
    // hands administrator rights to everything started from it afterwards.
    // Nothing here needs elevation: the registration is per-user.
    if elevated() {
        return Err("reinstall must run without administrator rights, or the \
                    restarted Explorer would keep them for the whole session"
            .into());
    }

    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let from = exe.parent().ok_or("the executable has no directory to install from")?.to_path_buf();
    // Checked here rather than left to `swap`, which is skipped when the
    // installation is already in this directory: registering a DLL that is not
    // there would leave Explorer loading nothing.
    for name in BINARIES {
        let path = from.join(name);
        if !path.is_file() {
            return Err(format!("{} is not there to install", path.display()));
        }
    }
    let registered = ro_register::is_installed();
    let was_registered = registered.is_some();
    let to = destination(registered, &from);

    // Anything an interrupted run left behind goes first, while the names are
    // free and before new ones are added to the pile.
    sweep(&to);
    if !same_directory(&from, &to) {
        swap(&from, &to).map_err(|e| format!("cannot install into {}: {e}", to.display()))?;
    }
    ro_register::install(&to.join(BINARIES[1]))
        .map_err(|e| format!("cannot register {}: {e}", to.display()))?;

    // Whether the files here changed is not the question: a package manager
    // may have rewritten them in place before calling this, and Explorer would
    // still be running the image it mapped earlier.
    if !was_registered {
        return Ok((to, false, Vec::new()));
    }
    explorer::restart().map_err(|e| {
        format!("{e}. The binaries are in place in {}; restart Explorer to load them", to.display())
    })?;
    Ok((to.clone(), true, sweep(&to)))
}

/// Where an installation already lives, or the source directory when there is
/// none — which makes the first `reinstall` an ordinary install.
///
/// A registration pointing somewhere that no longer exists is ignored rather
/// than followed: writing binaries into a directory that has been removed
/// would create a copy nothing knows about.
fn destination(registered: Option<PathBuf>, from: &Path) -> PathBuf {
    registered
        .as_deref()
        .and_then(Path::parent)
        .filter(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| from.to_path_buf())
}

/// Whether both names lead to the same directory, so nothing is copied over
/// itself. Compared after resolving, since the registry and `current_exe` need
/// not spell a path the same way.
fn same_directory(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_existing_registration_decides_where_the_binaries_go() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("ro_shellext.dll");
        std::fs::write(&installed, b"x").unwrap();
        let from = Path::new(r"C:\somewhere\else");
        assert_eq!(destination(Some(installed), from), dir.path());
    }

    #[test]
    fn without_a_registration_the_binaries_stay_where_they_are() {
        let from = Path::new(r"C:\somewhere\else");
        assert_eq!(destination(None, from), from);
    }

    /// An install that has since been deleted must not be recreated.
    #[test]
    fn a_registration_pointing_nowhere_is_ignored() {
        let from = Path::new(r"C:\somewhere\else");
        let gone = PathBuf::from(r"C:\no\such\directory\ro_shellext.dll");
        assert_eq!(destination(Some(gone), from), from);
    }

    #[test]
    fn a_directory_is_the_same_as_itself_however_it_is_spelled() {
        let dir = tempfile::tempdir().unwrap();
        let plain = dir.path().to_path_buf();
        let roundabout = plain.join("sub").join("..");
        std::fs::create_dir(plain.join("sub")).unwrap();
        assert!(same_directory(&plain, &roundabout));
        assert!(!same_directory(&plain, &plain.join("sub")));
    }
}
