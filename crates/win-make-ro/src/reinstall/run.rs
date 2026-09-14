use std::path::Path;

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
/// something was registered before, since a first install has nothing loaded to
/// replace, and never from an elevated process, which would hand the new shell
/// its token for the session.
pub fn reinstall(to: Option<&Path>) -> Result<Summary, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate own executable: {e}"))?;
    let from = exe.parent().ok_or("the executable has no directory to install from")?.to_path_buf();
    for name in Binaries::NAMES {
        let path = from.join(name);
        if !path.is_file() {
            return Err(format!("{} is not there to install", path.display()));
        }
    }
    let was_registered = ro_register::is_installed().is_some();

    let (to, mut superseded) = match to {
        None => (from, Vec::new()),
        Some(dir) => {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("cannot use {} as a destination: {e}", dir.display()))?;
            // Anything an interrupted run left behind goes first, while the
            // names are free and before new ones join the pile.
            sweep(dir, &[]);
            let made = swap(&from, dir)
                .map_err(|e| format!("cannot install into {}: {e}", dir.display()))?;
            (dir.to_path_buf(), made)
        }
    };

    ro_register::install(&to.join(Binaries::NAMES[1]))
        .map_err(|e| format!("cannot register {}: {e}", to.display()))?;

    // Whether files changed here is not the question: a package manager may
    // have rewritten them in place before calling this, and Explorer would
    // still be running the image it mapped earlier.
    let restart = if was_registered { explorer::restart() } else { Restart::NotRequested };
    if restart != Restart::Restarted {
        // The old copies are still mapped, so they stay for the next run.
        return Ok(Summary { to, restart, left_behind: superseded });
    }
    superseded = sweep(&to, &superseded);
    Ok(Summary { to, restart, left_behind: superseded })
}

#[cfg(test)]
mod tests {
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

    #[test]
    fn the_destination_is_not_made_until_the_source_is_whole() {
        let dir = tempfile::tempdir().unwrap();
        let to = dir.path().join("not").join("there").join("yet");
        // The source pair is checked first, so this fails before anything is
        // created anywhere.
        assert!(reinstall(Some(&to)).is_err());
        assert!(!to.exists(), "the destination is made only once the source is whole");
    }
}
