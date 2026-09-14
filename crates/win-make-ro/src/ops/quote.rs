use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

const QUOTE: u16 = b'"' as u16;
const BACKSLASH: u16 = b'\\' as u16;

/// Quotes one argument per the `CommandLineToArgvW` rules, in UTF-16.
///
/// Backslashes are literal except in a run immediately before a `"`, where
/// each must be doubled. The run before the closing quote counts, so a
/// trailing `\` in a path would otherwise escape it and swallow the next
/// argument. Working in UTF-16 keeps names that are not valid Unicode — an
/// unpaired surrogate is a legal NTFS file name — intact.
pub fn quote(arg: &OsStr) -> Vec<u16> {
    let mut out = Vec::with_capacity(arg.len() + 2);
    out.push(QUOTE);
    let mut backslashes = 0usize;
    for unit in arg.encode_wide() {
        match unit {
            BACKSLASH => {
                backslashes += 1;
                out.push(unit);
            }
            QUOTE => {
                // 2n + 1: double the run, then escape the quote itself.
                out.extend(std::iter::repeat_n(BACKSLASH, backslashes + 1));
                backslashes = 0;
                out.push(unit);
            }
            _ => {
                backslashes = 0;
                out.push(unit);
            }
        }
    }
    // Double the run that would otherwise escape the closing quote.
    out.extend(std::iter::repeat_n(BACKSLASH, backslashes));
    out.push(QUOTE);
    out
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::UI::Shell::CommandLineToArgvW;
    use windows::core::PCWSTR;

    use super::quote;

    /// Round-trips through the very parser the elevated child will use.
    fn argv(args: &[OsString]) -> Vec<OsString> {
        let mut line: Vec<u16> = "app.exe".encode_utf16().collect();
        for a in args {
            line.push(u16::from(b' '));
            line.extend_from_slice(&quote(a));
        }
        line.push(0);
        let mut count = 0i32;
        // SAFETY: line is NUL-terminated; the result is freed below.
        unsafe {
            let p = CommandLineToArgvW(PCWSTR(line.as_ptr()), &mut count);
            assert!(!p.is_null(), "CommandLineToArgvW failed");
            let out = (1..count as isize)
                .map(|i| {
                    let s = (*p.offset(i)).0;
                    let mut len = 0isize;
                    while *s.offset(len) != 0 {
                        len += 1;
                    }
                    OsString::from_wide(std::slice::from_raw_parts(s, len as usize))
                })
                .collect();
            let _ = LocalFree(Some(HLOCAL(p.cast())));
            out
        }
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn trailing_backslash_does_not_swallow_the_next_argument() {
        let args = os(&["lock", "--", r"C:\data\", r"C:\other"]);
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn plain_paths_and_spaces_round_trip() {
        let args = os(&[r"C:\a b\c.txt", "x", r"D:\", r"\\server\share\dir"]);
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn quotes_and_backslash_runs_round_trip() {
        let args = os(&[r#"a"b"#, r"c\\", r#"d\\"e"#, r"\\\"]);
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn empty_argument_survives() {
        assert_eq!(argv(&os(&["", "x"])), os(&["", "x"]));
    }

    /// An unpaired surrogate is a legal NTFS name and must not be folded into
    /// U+FFFD, which would make two distinct files indistinguishable.
    #[test]
    fn unpaired_surrogates_round_trip() {
        let a = OsString::from_wide(&[b'x' as u16, 0xD800]);
        let b = OsString::from_wide(&[b'x' as u16, 0xDC00]);
        assert_ne!(a, b);
        let args = vec![a, b];
        assert_eq!(argv(&args), args);
    }
}
