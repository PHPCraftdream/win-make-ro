#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    MakeReadOnly,
    RemoveReadOnly,
    /// Informational, disabled: everything selected is locked via a parent.
    InheritedInfo,
}

impl Item {
    pub fn text(self) -> &'static str {
        match self {
            Self::MakeReadOnly => "Make read only",
            Self::RemoveReadOnly => "Remove read only",
            Self::InheritedInfo => "Read only (inherited from parent folder)",
        }
    }

    pub fn verb(self) -> &'static str {
        match self {
            Self::MakeReadOnly => "makereadonly",
            Self::RemoveReadOnly => "removereadonly",
            Self::InheritedInfo => "readonlyinherited",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Self::MakeReadOnly => "Deny writes and deletes for everyone, recursively",
            Self::RemoveReadOnly => "Remove the read-only lock set by this tool, recursively",
            Self::InheritedInfo => "Unlock the parent folder to make this writable",
        }
    }

    pub fn enabled(self) -> bool {
        !matches!(self, Self::InheritedInfo)
    }

    /// Helper sub-command, if the item does anything.
    pub fn command(self) -> Option<&'static str> {
        match self {
            Self::MakeReadOnly => Some("lock"),
            Self::RemoveReadOnly => Some("unlock"),
            Self::InheritedInfo => None,
        }
    }
}
