use std::path::Path;

use ro_core::wide_path;

fn s(v: &[u16]) -> String {
    String::from_utf16(&v[..v.len() - 1]).unwrap()
}

#[test]
fn drive_path_gets_prefix_and_nul() {
    let v = wide_path(Path::new(r"C:\Windows")).unwrap();
    assert_eq!(*v.last().unwrap(), 0);
    assert_eq!(s(&v), r"\\?\C:\Windows");
}

#[test]
fn already_prefixed_path_is_untouched() {
    let v = wide_path(Path::new(r"\\?\C:\Windows")).unwrap();
    assert_eq!(s(&v), r"\\?\C:\Windows");
}

#[test]
fn unc_path_gets_unc_prefix() {
    let v = wide_path(Path::new(r"\\server\share\dir")).unwrap();
    assert_eq!(s(&v), r"\\?\UNC\server\share\dir");
}

#[test]
fn relative_path_is_made_absolute() {
    let v = wide_path(Path::new("x")).unwrap();
    let expect = std::path::absolute("x").unwrap();
    assert_eq!(s(&v), format!(r"\\?\{}", expect.display()));
}
