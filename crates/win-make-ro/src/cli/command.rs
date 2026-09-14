#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Lock,
    Unlock,
    Status,
    Install,
    Reinstall,
    Uninstall,
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "lock" => Self::Lock,
            "unlock" => Self::Unlock,
            "status" => Self::Status,
            "install" => Self::Install,
            "reinstall" => Self::Reinstall,
            "uninstall" => Self::Uninstall,
            _ => return None,
        })
    }

    pub fn takes_paths(self) -> bool {
        matches!(self, Self::Lock | Self::Unlock | Self::Status)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lock => "lock",
            Self::Unlock => "unlock",
            Self::Status => "status",
            Self::Install => "install",
            Self::Reinstall => "reinstall",
            Self::Uninstall => "uninstall",
        }
    }
}
