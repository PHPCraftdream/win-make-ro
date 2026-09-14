use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use ro_core::{ErrorKind, Report, lock_tree, unlock_tree};
use windows::Win32::UI::Shell::{
    SHCNE_UPDATEDIR, SHCNE_UPDATEITEM, SHCNF_FLUSH, SHCNF_FLUSHNOWAIT, SHCNF_PATHW, SHChangeNotify,
};

use crate::cli::Command;

/// How often the view is refreshed while the work is still going.
///
/// Windows seals a folder one file at a time, top to bottom, and a big one
/// takes seconds. Refreshing as it goes turns that into something a person can
/// watch — the read-only marks spread through the listing — instead of a wait
/// with no end in sight.
const REFRESH_EVERY: Duration = Duration::from_secs(1);

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
    let targets = ordered(paths);
    let total = {
        // Only while the work runs, and never for work that finishes inside
        // the first interval — a folder that locks in a fifth of a second
        // needs one refresh, not a flicker.
        let watched = targets.clone();
        let _ticking = Ticker::start(REFRESH_EVERY, move || {
            for path in &watched {
                tell_the_shell(path, Delivery::WhenItSuits);
            }
        });
        work(command, targets.clone())
    };
    // The last one is the one that has to land, and this process is about to
    // end: it waits for the shell to take it.
    for path in &targets {
        tell_the_shell(path, Delivery::BeforeWeGo);
    }
    total
}

fn work(command: Command, targets: Vec<PathBuf>) -> Report {
    let mut total = Report::default();
    let mut pending = targets;
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

/// Whether the notification has to be handed over before we carry on.
#[derive(Clone, Copy)]
enum Delivery {
    /// Blocks until the shell has taken it. For the last one, because this
    /// process is about to end and an undelivered message dies with it.
    BeforeWeGo,
    /// Left for the shell to pick up. For the ones sent while the work runs,
    /// where waiting would put the refresh in the way of the thing it is
    /// reporting on.
    WhenItSuits,
}

/// Tells Explorer that `path` has changed, so it re-reads it now rather than
/// whenever something else happens to make it look.
///
/// Both halves of the lock are invisible to a folder view that is not told:
/// the DACL is not drawn at all, and the READONLY attribute is drawn from a
/// cached listing. Without this the work finished in a fifth of a second and
/// the screen went on showing the old state for as long as the user happened
/// to wait — which is what "it takes a long time" meant.
///
/// Sent even when the operation failed: it may have failed part of the way
/// through, and a stale view is not a better picture of that.
fn tell_the_shell(path: &Path, delivery: Delivery) {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // A folder's contents are what changed; for a file it is the file itself.
    let what = if path.is_dir() { SHCNE_UPDATEDIR } else { SHCNE_UPDATEITEM };
    let how = match delivery {
        Delivery::BeforeWeGo => SHCNF_FLUSH,
        Delivery::WhenItSuits => SHCNF_FLUSHNOWAIT,
    };
    // SAFETY: SHCNF_PATHW says the first item is a NUL-terminated wide string,
    // which outlives the call, and that the second is unused.
    unsafe {
        SHChangeNotify(what, SHCNF_PATHW | how, Some(wide.as_ptr().cast()), None);
    }
}

/// Runs `tick` every `every` for as long as it is kept, and not once more.
///
/// Dropping it stops the ticking and waits for the thread, so nothing arrives
/// after the final notification — a refresh that lands later than the one
/// saying the work is done would show the state from before it finished.
struct Ticker {
    stop: Option<Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl Ticker {
    fn start(every: Duration, mut tick: impl FnMut() + Send + 'static) -> Self {
        let (stop, wait) = channel::<()>();
        let thread = std::thread::spawn(move || {
            // Waiting on the channel is the sleep: dropping the sender wakes
            // it at once rather than at the end of the interval.
            while wait.recv_timeout(every) == Err(RecvTimeoutError::Timeout) {
                tick();
            }
        });
        Self { stop: Some(stop), thread: Some(thread) }
    }
}

impl Drop for Ticker {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
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
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{Ticker, ordered};

    /// Work that finishes inside one interval gets no refresh of its own, only
    /// the final one. A small folder locks in a fifth of a second and must not
    /// flicker.
    #[test]
    fn nothing_ticks_for_work_that_is_over_quickly() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&ticks);
        drop(Ticker::start(std::time::Duration::from_secs(30), move || {
            counted.fetch_add(1, Ordering::Relaxed);
        }));
        assert_eq!(ticks.load(Ordering::Relaxed), 0);
    }

    /// Work that runs on gets a refresh each interval, and none once it is over:
    /// a notification arriving after the last one would redraw the state from
    /// before the work finished.
    #[test]
    fn it_ticks_while_the_work_runs_and_stops_with_it() {
        let ticks = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&ticks);
        let every = std::time::Duration::from_millis(40);
        let ticking = Ticker::start(every, move || {
            counted.fetch_add(1, Ordering::Relaxed);
        });
        std::thread::sleep(every * 5);
        drop(ticking);

        let while_running = ticks.load(Ordering::Relaxed);
        assert!(while_running >= 2, "only {while_running} refreshes in five intervals");
        std::thread::sleep(every * 3);
        assert_eq!(ticks.load(Ordering::Relaxed), while_running, "it ticked after being stopped");
    }

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
