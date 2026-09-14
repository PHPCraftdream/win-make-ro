use std::ffi::OsString;
use std::path::PathBuf;

use super::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub command: Command,
    pub paths: Vec<PathBuf>,
    pub gui: bool,
    /// Never re-launch elevated (also set on the elevated child).
    pub no_elevate: bool,
    /// Where `paths` came from, when a selection was too large for a command
    /// line. Whoever acts on the list removes the file afterwards.
    pub paths_file: Option<PathBuf>,
}

impl Args {
    /// Takes `OsString`s: a path that is not valid Unicode still has to survive
    /// intact, so nothing here goes through `String`.
    pub fn parse(mut it: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let word = it.next().ok_or("missing command")?;
        let command = word
            .to_str()
            .and_then(Command::parse)
            .ok_or_else(|| format!("unknown command: {}", word.to_string_lossy()))?;
        let mut args =
            Args { command, paths: Vec::new(), gui: false, no_elevate: false, paths_file: None };
        let mut opts_done = false;
        let mut want_paths_file = false;
        let mut typed_paths = false;
        for a in it {
            if want_paths_file {
                want_paths_file = false;
                let file = PathBuf::from(a);
                let listed = ro_core::read_paths_file(&file)
                    .map_err(|e| format!("--paths-from {}: {e}", file.display()))?;
                args.paths.extend(listed);
                args.paths_file = Some(file);
                continue;
            }
            // Only a valid-Unicode argument can be an option; anything else is
            // a path by definition.
            match a.to_str() {
                Some("--") if !opts_done => opts_done = true,
                Some("--gui") if !opts_done => args.gui = true,
                Some("--no-elevate") if !opts_done => args.no_elevate = true,
                Some("--paths-from") if !opts_done => {
                    if args.paths_file.is_some() {
                        return Err("--paths-from can only be given once".into());
                    }
                    want_paths_file = true;
                }
                Some(s) if !opts_done && s.starts_with("--") => {
                    return Err(format!("unknown option: {s}"));
                }
                _ => {
                    typed_paths = true;
                    args.paths.push(PathBuf::from(a));
                }
            }
        }
        if want_paths_file {
            return Err("--paths-from needs a file".into());
        }
        // The list is the whole selection: re-encoding for an elevated child
        // passes the file on rather than the paths in it, so a target named
        // beside it would be handed to nobody.
        if args.paths_file.is_some() && typed_paths {
            return Err("--paths-from cannot be combined with paths on the command line".into());
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
    pub fn to_argv(&self) -> Vec<OsString> {
        let mut v = vec![OsString::from(self.command.as_str())];
        if self.gui {
            v.push("--gui".into());
        }
        if self.no_elevate {
            v.push("--no-elevate".into());
        }
        // The list stays in the file when there is one: re-expanding it on the
        // command line is what did not fit in the first place.
        if let Some(file) = &self.paths_file {
            v.push("--paths-from".into());
            v.push(file.clone().into_os_string());
            return v;
        }
        v.push("--".into());
        v.extend(self.paths.iter().map(|p| p.clone().into_os_string()));
        v
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::ffi::OsStringExt;

    use super::*;

    fn parse(s: &[&str]) -> Result<Args, String> {
        Args::parse(s.iter().map(OsString::from))
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

    fn list_file(paths: &[&str]) -> PathBuf {
        ro_core::write_paths_file(&paths.iter().map(PathBuf::from).collect::<Vec<_>>()).unwrap()
    }

    /// `--paths-from` carries the whole selection, and `to_argv` re-emits the
    /// file rather than the paths in it. Anything named beside it would be
    /// dropped on the way to an elevated child — and a second list would lose
    /// everything but its own name — so both shapes are refused here instead.
    #[test]
    fn a_path_list_cannot_be_mixed_with_other_targets() {
        let list = list_file(&[r"C:\listed.txt"]);
        let name = list.to_str().expect("temp dir is plain text").to_string();
        assert!(parse(&["lock", "--paths-from", &name, r"C:\extra.txt"]).is_err());
        assert!(parse(&["lock", r"C:\extra.txt", "--paths-from", &name]).is_err());
        assert!(parse(&["lock", "--paths-from", &name, "--paths-from", &name]).is_err());
        assert!(parse(&["lock", "--paths-from", &name]).is_ok());
        ro_core::remove_paths_file(&list);
    }

    /// What the child is handed has to name every target the parent was given.
    #[test]
    fn a_path_list_survives_re_encoding_for_a_child() {
        let list = list_file(&[r"C:\one.txt", r"C:\two.txt"]);
        let name = list.to_str().expect("temp dir is plain text").to_string();
        let a = parse(&["lock", "--gui", "--paths-from", &name]).unwrap();
        assert_eq!(a.paths.len(), 2);
        assert_eq!(Args::parse(a.to_argv().into_iter()).unwrap(), a);
        ro_core::remove_paths_file(&list);
    }

    /// A path that is not valid Unicode must survive parsing and re-encoding.
    #[test]
    fn lone_surrogate_path_survives() {
        let odd = OsString::from_wide(&[b'x' as u16, 0xD800]);
        let a =
            Args::parse([OsString::from("lock"), OsString::from("--"), odd.clone()].into_iter())
                .unwrap();
        assert_eq!(a.paths, vec![PathBuf::from(&odd)]);
        assert_eq!(Args::parse(a.to_argv().into_iter()).unwrap(), a);
    }
}
