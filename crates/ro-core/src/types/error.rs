use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::WIN32_ERROR;

use super::ErrorKind;

#[derive(Debug)]
pub struct Error {
    pub path: PathBuf,
    pub kind: ErrorKind,
}

impl Error {
    pub fn new(path: &Path, kind: ErrorKind) -> Self {
        Self { path: path.to_path_buf(), kind }
    }

    pub fn os(path: &Path, err: io::Error) -> Self {
        Self::new(path, ErrorKind::Os(err))
    }

    pub fn win(path: &Path, code: WIN32_ERROR) -> Self {
        Self::os(path, io::Error::from_raw_os_error(code.0 as i32))
    }

    pub fn is_access_denied(&self) -> bool {
        const ERROR_ACCESS_DENIED: i32 = 5;
        matches!(&self.kind, ErrorKind::Os(e) if e.raw_os_error() == Some(ERROR_ACCESS_DENIED))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ErrorKind::Os(e) => write!(f, "{}: {e}", self.path.display()),
            ErrorKind::LockedByParent => {
                write!(f, "{}: locked by a parent folder, unlock it instead", self.path.display())
            }
            ErrorKind::ReparsePoint => {
                write!(f, "{}: symlink/junction skipped", self.path.display())
            }
            ErrorKind::NullDacl => write!(
                f,
                "{}: has no access control list at all (NULL DACL); locking it could not be                  undone exactly, so it was left unchanged",
                self.path.display()
            ),
        }
    }
}

impl std::error::Error for Error {}
