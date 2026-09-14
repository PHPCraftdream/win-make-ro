use std::fs::File;
use std::io::{self, Write};
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Writes `paths` to a fresh file in the temp directory and returns it.
///
/// The caller hands the name to the process that will act on the list; that
/// process removes the file once it has read it.
pub fn write_paths_file(paths: &[PathBuf]) -> io::Result<PathBuf> {
    let dir = std::env::temp_dir();
    let mut units: Vec<u16> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        if i > 0 {
            units.push(0);
        }
        units.extend(p.as_os_str().encode_wide());
    }
    let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_le_bytes()).collect();

    // create_new so a name that is somehow taken is never overwritten.
    for attempt in 0..16u32 {
        let path = dir.join(name(attempt));
        match File::create_new(&path) {
            Ok(mut f) => {
                f.write_all(&bytes)?;
                return Ok(path);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "no free name for the path list"))
}

fn name(attempt: u32) -> String {
    let stamp =
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("win-make-ro-{}-{stamp}-{attempt}.paths", std::process::id())
}

/// Removes a list written by [`write_paths_file`]. A list that is already gone
/// is not an error: the process that read it deletes it too.
pub fn remove_paths_file(path: &Path) {
    let _ = std::fs::remove_file(path);
}
