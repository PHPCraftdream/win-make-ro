use std::io;
use std::path::{Path, PathBuf};

use super::{remove_paths_file, write_paths_file};

/// A path list that lives no longer than the code which created it.
///
/// The list exists only to carry a selection to another process, and the way
/// out of that code is not always the happy one: a refused elevation prompt, a
/// spawn that failed, an early `return` added later. Tying the file to a value
/// rather than to a particular line means every one of those takes it along,
/// instead of leaving it in the temp directory for good.
pub struct PathsFile {
    path: PathBuf,
}

impl PathsFile {
    pub fn new(paths: &[PathBuf]) -> io::Result<Self> {
        Ok(Self { path: write_paths_file(paths)? })
    }

    /// The name to hand to whoever will read the list.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PathsFile {
    fn drop(&mut self) {
        remove_paths_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_goes_when_the_guard_does() {
        let name;
        {
            let list = PathsFile::new(&[PathBuf::from(r"C:\a.txt")]).unwrap();
            name = list.path().to_path_buf();
            assert!(name.is_file(), "the list was not written");
        }
        assert!(!name.exists(), "the list outlived its owner");
    }
}
