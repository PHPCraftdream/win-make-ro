/// Quotes one argument per the `CommandLineToArgvW` rules: backslashes are
/// literal except in a run immediately before a `"`, where each must be
/// doubled. The run before the closing quote counts, so a trailing `\` in a
/// path would otherwise escape it and swallow the next argument.
pub fn quote(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => {
                backslashes += 1;
                out.push(c);
            }
            '"' => {
                // 2n + 1: double the run, then escape the quote itself.
                for _ in 0..=backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            }
            _ => {
                backslashes = 0;
                out.push(c);
            }
        }
    }
    // Double the run that would otherwise escape the closing quote.
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::UI::Shell::CommandLineToArgvW;
    use windows::core::PCWSTR;

    use super::quote;

    /// Round-trips through the very parser the elevated child will use.
    fn argv(args: &[&str]) -> Vec<String> {
        let line =
            format!("app.exe {}", args.iter().map(|a| quote(a)).collect::<Vec<_>>().join(" "));
        let wide: Vec<u16> = line.encode_utf16().chain(Some(0)).collect();
        let mut count = 0i32;
        // SAFETY: wide is NUL-terminated; the result is freed below.
        unsafe {
            let p = CommandLineToArgvW(PCWSTR(wide.as_ptr()), &mut count);
            assert!(!p.is_null(), "CommandLineToArgvW failed");
            let out =
                (1..count as isize).map(|i| (*p.offset(i)).to_string().expect("utf16")).collect();
            let _ = LocalFree(Some(HLOCAL(p.cast())));
            out
        }
    }

    #[test]
    fn trailing_backslash_does_not_swallow_the_next_argument() {
        let args = ["lock", "--", r"C:\data\", r"C:\other"];
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn plain_paths_and_spaces_round_trip() {
        let args = [r"C:\a b\c.txt", "x", r"D:\", r"\\server\share\dir"];
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn quotes_and_backslash_runs_round_trip() {
        let args = [r#"a"b"#, r"c\\", r#"d\\"e"#, r"\\\"];
        assert_eq!(argv(&args), args);
    }

    #[test]
    fn empty_argument_survives() {
        assert_eq!(argv(&["", "x"]), ["", "x"]);
    }
}
