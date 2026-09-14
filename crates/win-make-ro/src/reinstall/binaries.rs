/// What an installation is made of, and what its temporary copies are called.
///
/// The names live together because the sweep deletes files by recognising
/// them: a shape invented in one place and matched in another is how a
/// stranger's backup ends up in the wrong bucket.
pub struct Binaries;

impl Binaries {
    /// The two files, in the order they are replaced. The helper goes first:
    /// the DLL is the one Explorer holds open, so a failure while putting it in
    /// place has less to undo.
    pub const NAMES: [&'static str; 2] = ["win-make-ro.exe", "ro_shellext.dll"];

    /// A new copy, written before anything in place is disturbed.
    pub fn staged(binary: &str, stamp: &str) -> String {
        format!("{binary}.{MARK}-{stamp}.new")
    }

    /// A copy that has been replaced and is waiting to be deleted.
    pub fn superseded(binary: &str, stamp: &str) -> String {
        format!("{binary}.{MARK}-{stamp}.old")
    }

    /// Whether `name` is exactly what [`Binaries::superseded`] produces.
    ///
    /// Narrow on purpose. `ro_shellext.dll.old` and
    /// `ro_shellext.dll.backup.old` are names a person writes by hand, and
    /// whatever matches this gets deleted.
    pub fn is_superseded(name: &str) -> bool {
        let Some(stamp) = name
            .strip_suffix(".old")
            .and_then(|rest| Self::NAMES.iter().find_map(|b| rest.strip_prefix(*b)))
            .and_then(|rest| rest.strip_prefix(&format!(".{MARK}-")))
        else {
            return false;
        };
        // A process id and a nanosecond count, both digits and neither empty.
        matches!(stamp.split_once('-'), Some((pid, nanos))
            if !pid.is_empty()
                && !nanos.is_empty()
                && pid.bytes().all(|b| b.is_ascii_digit())
                && nanos.bytes().all(|b| b.is_ascii_digit()))
    }
}

/// Long and unmistakable: it is the difference between our copy and someone's.
const MARK: &str = "superseded";

#[cfg(test)]
mod tests {
    use super::Binaries;

    #[test]
    fn a_name_this_tool_made_is_recognised() {
        for binary in Binaries::NAMES {
            assert!(Binaries::is_superseded(&Binaries::superseded(binary, "123-456")));
        }
    }

    #[test]
    fn a_name_a_person_would_write_is_not() {
        for name in [
            "ro_shellext.dll",
            "ro_shellext.dll.old",
            "ro_shellext.dll.backup.old",
            "ro_shellext.dll.superseded.old",
            "ro_shellext.dll.superseded-.old",
            "ro_shellext.dll.superseded-a-b.old",
            "ro_shellext.dll.superseded-1.old",
            "something.dll.superseded-1-2.old",
            "notes.old",
            // The staged half of an exchange is not a superseded copy.
            "ro_shellext.dll.superseded-1-2.new",
        ] {
            assert!(!Binaries::is_superseded(name), "{name} would have been deleted");
        }
    }
}
