use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// How much of a command line we are willing to use.
///
/// `CreateProcessW` refuses one longer than 32 767 characters, so the limit is
/// approached with room to spare for the executable path, the verb and the
/// quoting each argument picks up.
/// See <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw>.
pub const COMMAND_LINE_BUDGET: usize = 30_000;

/// Whether `paths` still fit on a command line alongside `fixed` characters of
/// executable path and options. Each path is counted with the two quotes and
/// the separating space it will be given.
pub fn fits_command_line<P: AsRef<Path>>(fixed: usize, paths: &[P]) -> bool {
    let mut total = fixed;
    for p in paths {
        total = total.saturating_add(p.as_ref().as_os_str().encode_wide().count() + 3);
        if total > COMMAND_LINE_BUDGET {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{COMMAND_LINE_BUDGET, fits_command_line};

    #[test]
    fn a_small_selection_fits() {
        let paths = vec![PathBuf::from(r"C:\a.txt"), PathBuf::from(r"C:\b.txt")];
        assert!(fits_command_line(64, &paths));
    }

    #[test]
    fn a_large_selection_does_not() {
        let one = PathBuf::from(format!(r"C:\{}.txt", "p".repeat(180)));
        let paths = vec![one; 180];
        assert!(!fits_command_line(64, &paths));
    }

    #[test]
    fn the_budget_is_counted_in_utf16_units() {
        // One path just over the budget on its own.
        let long = PathBuf::from("x".repeat(COMMAND_LINE_BUDGET));
        assert!(!fits_command_line(0, &[long]));
    }
}
