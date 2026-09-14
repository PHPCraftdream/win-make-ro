use std::path::{Path, PathBuf};

use super::binaries::Binaries;
use super::explorer;
use super::outcome::{Restart, Summary};
use super::swap::swap;
use super::sweep::sweep;

/// Makes an installation current and restarts Explorer so it is the one in use.
///
/// `to` names a directory to install *into*, which is the only way the binaries
/// beside this executable are copied anywhere: the superseded pair is renamed
/// aside — a rename moves the directory entry, not the mapping a running
/// Explorer holds — and deleted once Explorer has gone. Without it the pair
/// beside this executable is simply registered where it already is.
///
/// That distinction is the whole point. An earlier version took the destination
/// from whatever was registered, so an `npm install -g` on a machine carrying a
/// Scoop or hand-made installation wrote its binaries into that other
/// directory and left the registration pointing there — where the other
/// installer would later delete them.
///
/// Three decisions, kept apart: where to install, whether a restart is worth
/// asking for, and whether it may be carried out. Explorer is restarted only if
/// something was registered before — the practical signal, not a measurement:
/// an earlier `uninstall` leaves the DLL mapped and the registry empty. And
/// never from an elevated process, which would hand the new shell its token for
/// the session.
pub fn reinstall(to: Option<&Path>) -> Result<Summary, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let from = exe.parent().ok_or("the executable has no directory to install from")?;
    install_from(from, to)
}

/// The same, for a pair that is somewhere other than beside this process.
///
/// Split out so the order of the steps can be tested: everything up to the
/// registration is ordinary file work, and a test can make it fail on purpose
/// and look at what the destination is left holding.
fn install_from(from: &Path, to: Option<&Path>) -> Result<Summary, String> {
    for name in Binaries::NAMES {
        let path = from.join(name);
        if !path.is_file() {
            return Err(format!("{} is not there to install", path.display()));
        }
    }
    let was_registered = ro_register::is_installed().is_some();

    let target =
        destination(to, from).map_err(|e| format!("cannot work out where to install: {e}"))?;
    let mut superseded = Vec::new();
    if to.is_some() {
        std::fs::create_dir_all(&target)
            .map_err(|e| format!("cannot use {} as a destination: {e}", target.display()))?;
        superseded = swap(from, &target)
            .map_err(|e| format!("cannot install into {}: {e}", target.display()))?;
    }
    let to = target;

    ro_register::install(&to.join(Binaries::NAMES[1]))
        .map_err(|e| format!("cannot register {}: {e}", to.display()))?;

    // Whether files changed here is not the question: a package manager may
    // have rewritten them in place before calling this, and Explorer would
    // still be running the image it mapped earlier.
    let restart = if was_registered { explorer::restart() } else { Restart::NotRequested };
    if restart != Restart::Restarted {
        // The old copies may still be mapped, so they stay for the next run.
        return Ok(Summary { to, restart, left_behind: superseded });
    }
    // Only here, with the new pair in place and registered and the process
    // that held the old one gone. A superseded copy left by an earlier run may
    // be the last intact one — if that run's rollback could not put it back —
    // and deleting it before an exchange that then fails would take the only
    // thing left to recover from.
    let left_behind = sweep(&to, &superseded);
    Ok(Summary { to, restart, left_behind })
}

/// Where the binaries end up.
///
/// A named destination is made absolute here and nowhere else. The path goes
/// into `InprocServer32` as written, and a relative one would leave Explorer
/// looking for the DLL beside whatever its own working directory happens to
/// be — which works from the window the command was typed in, and stops
/// working at the next sign-in.
fn destination(to: Option<&Path>, from: &Path) -> std::io::Result<PathBuf> {
    match to {
        None => Ok(from.to_path_buf()),
        Some(dir) => std::path::absolute(dir),
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::fs::OpenOptionsExt;

    use super::*;

    /// The paths a foreign installation would sit at are not even looked up
    /// when no destination is named, so they cannot be written to.
    #[test]
    fn without_a_destination_nothing_is_copied_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let foreign = dir.path().join("scoop-apps-win-make-ro-1.0.0");
        std::fs::create_dir(&foreign).unwrap();
        for name in Binaries::NAMES {
            std::fs::write(foreign.join(name), b"scoop").unwrap();
        }

        // `reinstall(None)` installs beside the running executable, which for
        // this test binary holds neither of our two names — so it refuses
        // before touching anything, and the foreign copy is untouched either
        // way. What matters is that no path here reaches `foreign`.
        let err = reinstall(None).unwrap_err();
        assert!(err.contains("is not there to install"), "{err}");
        for name in Binaries::NAMES {
            assert_eq!(std::fs::read(foreign.join(name)).unwrap(), b"scoop", "{name} was written");
        }
    }

    /// The destination is written into the registry, and a COM host does not
    /// inherit the working directory the command was typed in. `--to dist`
    /// used to register `dist\ro_shellext.dll`, which Explorer finds from that
    /// one window and nowhere else.
    #[test]
    fn a_named_destination_is_made_absolute_before_anything_uses_it() {
        let from = Path::new(r"C:\beside\the\executable");
        let relative = destination(Some(Path::new("dist")), from).unwrap();
        assert!(relative.is_absolute(), "{} is relative", relative.display());
        assert!(relative.ends_with("dist"), "{} is not the named directory", relative.display());

        let already = destination(Some(Path::new(r"D:\elsewhere")), from).unwrap();
        assert_eq!(already, Path::new(r"D:\elsewhere"), "an absolute one is left alone");

        // Nothing named: the executable's own directory, which `current_exe`
        // already gives absolute.
        assert_eq!(destination(None, from).unwrap(), from);
    }

    #[test]
    fn the_destination_is_not_made_until_the_source_is_whole() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("not").join("there").join("yet");
        // The source pair is checked first, so this fails before anything is
        // created anywhere.
        assert!(reinstall(Some(&to)).is_err());
        assert!(!to.exists(), "the destination is made only once the source is whole");
    }

    /// A superseded copy is not spare rubbish: if an earlier rollback could not
    /// put it back under its own name, it is the only intact one left. Clearing
    /// the destination before an exchange that then fails took exactly that.
    ///
    /// The exchange is made to fail for real — the source DLL is open with no
    /// sharing, so `is_file` says yes and the copy says no — which stops this
    /// well before the registration, and no registry key or Explorer is
    /// touched.
    #[test]
    fn a_recovery_copy_survives_an_exchange_that_fails() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("new");
        let to = dir.path().join("installed");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        for name in Binaries::NAMES {
            std::fs::write(from.join(name), b"new").unwrap();
        }
        // All that is left of an interrupted run: a backup and no DLL.
        let backup = to.join("ro_shellext.dll.superseded-1-2.old");
        std::fs::write(&backup, b"the only intact copy").unwrap();

        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(from.join(Binaries::NAMES[1]))
            .expect("lock the source DLL");
        let err = install_from(&from, Some(&to)).unwrap_err();
        drop(locked);

        assert!(err.contains("cannot install into"), "{err}");
        assert!(backup.is_file(), "the last copy was swept away before the exchange");
        assert_eq!(std::fs::read(&backup).unwrap(), b"the only intact copy");
    }
}
