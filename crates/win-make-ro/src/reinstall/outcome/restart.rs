/// What became of Explorer.
///
/// Deliberately not a `bool`. "Explorer was restarted" and "we never looked"
/// and "we looked and it went wrong" are three different things to tell
/// somebody, and the last one has to be distinguishable from success even
/// though the installation itself is already in place.
#[derive(Debug, PartialEq, Eq)]
pub enum Restart {
    /// Nothing was registered before this ran, so there was no reason to ask.
    /// That is all it means: an earlier `uninstall` leaves the DLL mapped and
    /// the registry empty, so an empty registry is not a measurement of what
    /// Explorer is holding.
    NotRequested,
    /// Deliberately not attempted, with the reason: an elevated process would
    /// hand the restarted Explorer its token for the rest of the session, and
    /// a token we could not read is treated the same way.
    Skipped(String),
    /// Under way, and longer than we were prepared to watch. The request to
    /// close cannot be withdrawn, so this is not a cancellation: the process
    /// holding it will start the shell again whenever Explorer gets round to
    /// closing.
    Pending(String),
    /// The old shell process ended and a new one was started.
    Restarted,
    /// Asked for and did not happen. The installation is still in place.
    Failed(String),
}

impl Restart {
    /// One line for a person, in the imperative of what actually occurred.
    pub fn describe(&self) -> String {
        match self {
            Self::NotRequested => "no previous registration; Explorer restart not requested".into(),
            Self::Skipped(why) => format!("Explorer not restarted: {why}"),
            Self::Pending(what) => format!("Explorer restart still running: {what}"),
            Self::Restarted => "Explorer restarted".into(),
            Self::Failed(why) => format!("Explorer restart failed: {why}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Restart;

    /// The wording used to be "nothing was loaded to replace", which claims
    /// more than an empty registry can show: an earlier `uninstall` leaves the
    /// DLL mapped and the keys gone. What is actually known is that nobody
    /// asked.
    #[test]
    fn a_restart_that_was_never_asked_for_does_not_claim_anything_else() {
        let said = Restart::NotRequested.describe();
        assert_eq!(said, "no previous registration; Explorer restart not requested");
        assert!(!said.contains("loaded"), "{said}");
    }

    /// Only one of the five may read as a restart having happened.
    #[test]
    fn only_a_real_restart_says_it_restarted() {
        assert_eq!(Restart::Restarted.describe(), "Explorer restarted");
        for other in [
            Restart::NotRequested,
            Restart::Skipped("elevated".into()),
            Restart::Pending("process 7".into()),
            Restart::Failed("it did not close".into()),
        ] {
            let said = other.describe();
            assert!(!said.starts_with("Explorer restarted"), "{said}");
        }
    }

    /// Running out of patience is not the same as the work stopping. The
    /// request to close is already queued and cannot be taken back, so a
    /// caller that walked away has to say the exchange is still going — and
    /// must not read as a failure, which would have the caller retry.
    #[test]
    fn running_out_of_patience_does_not_read_as_a_failure() {
        let said = Restart::Pending("process 7".into()).describe();
        assert!(said.contains("still running"), "{said}");
        assert!(!said.contains("failed"), "{said}");
        assert!(!said.contains("not restarted"), "{said}");
    }
}
