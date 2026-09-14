use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use ro_core::PathsFile;
use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{
    SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW,
};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::PCWSTR;

use super::{quote, show_error};
use crate::cli::{Args, EXIT_ERRORS};

/// Re-runs this executable elevated (UAC) with `--no-elevate` and returns
/// the child's exit code. The operations are idempotent, so re-running the
/// whole set is safe.
pub fn elevate(args: &Args) -> i32 {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            show_error(args.gui, &format!("cannot locate own executable: {e}"));
            return EXIT_ERRORS;
        }
    };
    // `list` is held to the end of this function on purpose: it owns the file
    // written below, and every way out of here — a refused prompt included —
    // has to take it along.
    let (child, _list) = match child_args(args, &exe) {
        Ok(v) => v,
        Err(e) => {
            show_error(args.gui, &format!("cannot pass the selection on: {e}"));
            return EXIT_ERRORS;
        }
    };
    let wide = |s: &OsStr| -> Vec<u16> { s.encode_wide().chain(Some(0)).collect() };
    let verb = wide(OsStr::new("runas"));
    let file = wide(exe.as_os_str());
    // Built in UTF-16: a path that is not valid Unicode must reach the child
    // unchanged, so the command line is never routed through a String.
    let mut params: Vec<u16> = Vec::new();
    for a in child.to_argv() {
        if !params.is_empty() {
            params.push(u16::from(b' '));
        }
        params.extend_from_slice(&quote(&a));
    }
    params.push(0);

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpVerb: PCWSTR(verb.as_ptr()),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: all string buffers are NUL-terminated and outlive the call.
    if let Err(e) = unsafe { ShellExecuteExW(&mut info) } {
        if e.code().0 & 0xFFFF == ERROR_CANCELLED.0 as i32 {
            show_error(args.gui, "elevation was cancelled; nothing more was changed");
        } else {
            show_error(args.gui, &format!("elevation failed: {e}"));
        }
        return EXIT_ERRORS;
    }
    let mut code = EXIT_ERRORS as u32;
    // SAFETY: hProcess is a live handle we own (SEE_MASK_NOCLOSEPROCESS).
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
    }
    code as i32
}

/// Builds the arguments for the elevated child.
///
/// A selection can outgrow a command line, and the child would then be handed a
/// truncated list, so past that point it travels in a file. The guard that
/// comes back owns that file; a list the caller was itself given arrives with
/// an owner already and is only passed along.
fn child_args(args: &Args, exe: &Path) -> io::Result<(Args, Option<PathsFile>)> {
    let mut child = args.clone();
    child.no_elevate = true;
    if child.paths_file.is_some()
        || ro_core::fits_command_line(exe.as_os_str().encode_wide().count() + 64, &child.paths)
    {
        return Ok((child, None));
    }
    let list = PathsFile::new(&child.paths)?;
    child.paths_file = Some(list.path().to_path_buf());
    Ok((child, Some(list)))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::Command;

    const EXE: &str = r"C:\win-make-ro.exe";

    fn args_for(paths: Vec<PathBuf>) -> Args {
        Args { command: Command::Lock, paths, gui: false, no_elevate: false, paths_file: None }
    }

    /// The list written for the child belongs to a guard, so a prompt the user
    /// cancels — or any other early return — does not leave it in the temp
    /// directory. The prompt itself is not part of this: it cannot be shown
    /// without a real elevation, only the ownership is checked here.
    #[test]
    fn the_list_written_for_the_child_is_owned() {
        let long = PathBuf::from(format!(r"C:\{}.txt", "p".repeat(180)));
        let args = args_for(vec![long; 180]);
        let name;
        {
            let (child, list) = child_args(&args, Path::new(EXE)).unwrap();
            let list = list.expect("the fixture is not larger than a command line");
            name = list.path().to_path_buf();
            assert!(name.is_file());
            assert_eq!(child.paths_file.as_deref(), Some(name.as_path()));
            assert!(child.no_elevate, "the child must not elevate again");
        }
        assert!(!name.exists(), "the list outlived the elevation attempt");
    }

    /// A selection that fits is handed over as it is, with no file to clean up.
    #[test]
    fn a_selection_that_fits_needs_no_list() {
        let args = args_for(vec![PathBuf::from(r"C:\a.txt")]);
        let (child, list) = child_args(&args, Path::new(EXE)).unwrap();
        assert!(list.is_none());
        assert!(child.paths_file.is_none());
    }
}
