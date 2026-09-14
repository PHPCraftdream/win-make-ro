/// The two files an installation consists of, in the order they are replaced.
///
/// The helper goes first: if anything fails while the DLL is being put in
/// place, the rollback has less to undo, and the DLL is the one Explorer holds
/// open.
pub const BINARIES: [&str; 2] = ["win-make-ro.exe", "ro_shellext.dll"];
