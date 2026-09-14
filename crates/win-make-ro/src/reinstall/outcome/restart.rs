/// What became of Explorer.
///
/// Deliberately not a `bool`. "Explorer was restarted" and "we never looked"
/// and "we looked and it went wrong" are three different things to tell
/// somebody, and the last one has to be distinguishable from success even
/// though the installation itself is already in place.
#[derive(Debug, PartialEq, Eq)]
pub enum Restart {
    /// Nothing was registered before this ran, so no Explorer can be holding a
    /// copy of ours. Says nothing about what Explorer has mapped — an earlier
    /// `uninstall` leaves the DLL loaded and the registry empty — only that
    /// there was no reason to ask.
    NotRequested,
    /// Deliberately not attempted, with the reason: an elevated process would
    /// hand the restarted Explorer its token for the rest of the session, and
    /// a token we could not read is treated the same way.
    Skipped(String),
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

    /// Only one of the four may read as a restart having happened.
    #[test]
    fn only_a_real_restart_says_it_restarted() {
        assert_eq!(Restart::Restarted.describe(), "Explorer restarted");
        for other in [
            Restart::NotRequested,
            Restart::Skipped("elevated".into()),
            Restart::Failed("it did not close".into()),
        ] {
            let said = other.describe();
            assert!(!said.starts_with("Explorer restarted"), "{said}");
        }
    }
}
