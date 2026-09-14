use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

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
    let mut child = args.clone();
    child.no_elevate = true;
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            show_error(args.gui, &format!("cannot locate own executable: {e}"));
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
