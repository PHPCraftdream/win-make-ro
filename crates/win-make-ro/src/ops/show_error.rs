use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::PCWSTR;

/// stderr, or a message box when launched from the shell menu.
pub fn show_error(gui: bool, text: &str) {
    if !gui {
        eprintln!("{text}");
        return;
    }
    let wide = |s: &str| -> Vec<u16> { OsStr::new(s).encode_wide().chain(Some(0)).collect() };
    let body = wide(text);
    let title = wide("win-make-ro");
    // SAFETY: both buffers are NUL-terminated and outlive the call.
    unsafe {
        MessageBoxW(None, PCWSTR(body.as_ptr()), PCWSTR(title.as_ptr()), MB_OK | MB_ICONERROR)
    };
}
