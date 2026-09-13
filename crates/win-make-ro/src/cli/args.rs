use std::path::PathBuf;

use super::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub command: Command,
    pub paths: Vec<PathBuf>,
    pub gui: bool,
    /// Never re-launch elevated (also set on the elevated child).
    pub no_elevate: bool,
}

impl Args {
    pub fn parse(mut it: impl Iterator<Item = String>) -> Result<Self, String> {
        let word = it.next().ok_or("missing command")?;
        let command = Command::parse(&word).ok_or_else(|| format!("unknown command: {word}"))?;
        let mut args = Args { command, paths: Vec::new(), gui: false, no_elevate: false };
        let mut opts_done = false;
        for a in it {
            match a.as_str() {
                "--" if !opts_done => opts_done = true,
                "--gui" if !opts_done => args.gui = true,
                "--no-elevate" if !opts_done => args.no_elevate = true,
                s if s.starts_with("--") && !opts_done => {
                    return Err(format!("unknown option: {s}"));
                }
                _ => args.paths.push(PathBuf::from(a)),
            }
        }
        if command.takes_paths() && args.paths.is_empty() {
            return Err(format!("{}: at least one path required", command.as_str()));
        }
        if !command.takes_paths() && !args.paths.is_empty() {
            return Err(format!("{}: takes no paths", command.as_str()));
        }
        Ok(args)
    }

    /// Re-encodes for a child process; options first, then `--`, then paths.
    pub fn to_argv(&self) -> Vec<String> {
        let mut v = vec![self.command.as_str().to_string()];
        if self.gui {
            v.push("--gui".into());
        }
        if self.no_elevate {
            v.push("--no-elevate".into());
        }
        v.push("--".into());
        v.extend(self.paths.iter().map(|p| p.to_string_lossy().into_owned()));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &[&str]) -> Result<Args, String> {
        Args::parse(s.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_command_options_and_paths() {
        let a = parse(&["lock", "--gui", "C:\\a", "D:\\b"]).unwrap();
        assert_eq!(a.command, Command::Lock);
        assert!(a.gui);
        assert!(!a.no_elevate);
        assert_eq!(a.paths, vec![PathBuf::from("C:\\a"), PathBuf::from("D:\\b")]);
    }

    #[test]
    fn double_dash_ends_options() {
        let a = parse(&["unlock", "--", "--gui"]).unwrap();
        assert!(!a.gui);
        assert_eq!(a.paths, vec![PathBuf::from("--gui")]);
    }

    #[test]
    fn rejects_unknown_command_option_and_missing_paths() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["frobnicate"]).is_err());
        assert!(parse(&["lock", "--nope", "x"]).is_err());
        assert!(parse(&["lock"]).is_err());
        assert!(parse(&["install", "x"]).is_err());
        assert!(parse(&["install"]).is_ok());
    }

    #[test]
    fn argv_round_trips() {
        let a = parse(&["unlock", "--gui", "--no-elevate", "--", "C:\\a b", "--x"]).unwrap();
        let b = Args::parse(a.to_argv().into_iter()).unwrap();
        assert_eq!(a, b);
    }
}
