//! End-to-end: drives the built executable on a temp tree.

use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const EXE: &str = env!("CARGO_BIN_EXE_win-make-ro");

struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = run(&["unlock", "--no-elevate"], &[&self.0]);
    }
}

fn run(args: &[&str], paths: &[&Path]) -> Output {
    Command::new(EXE).args(args).arg("--").args(paths).output().expect("spawn helper")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn tree() -> (tempfile::TempDir, Guard, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let f = root.join("sub").join("f.txt");
    fs::create_dir_all(f.parent().unwrap()).unwrap();
    fs::write(&f, b"x").unwrap();
    let guard = Guard(root.clone());
    (dir, guard, root, f)
}

#[test]
fn lock_status_unlock_cycle() {
    let (_d, _g, root, f) = tree();

    let o = run(&["status"], &[&root, &f]);
    assert!(o.status.success());
    assert_eq!(stdout(&o), format!("unlocked\t{}\nunlocked\t{}\n", root.display(), f.display()));

    let o = run(&["lock", "--no-elevate"], &[&root]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(stdout(&o).is_empty(), "silent on success");

    let o = run(&["status"], &[&root, &f]);
    assert_eq!(stdout(&o), format!("locked\t{}\ninherited\t{}\n", root.display(), f.display()));
    assert!(fs::write(&f, b"y").is_err());

    let o = run(&["unlock", "--no-elevate"], &[&root]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let o = run(&["status"], &[&f]);
    assert_eq!(stdout(&o), format!("unlocked\t{}\n", f.display()));
    fs::write(&f, b"y").unwrap();
}

#[test]
fn multiple_paths_are_all_processed() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    fs::write(&a, b"a").unwrap();
    fs::write(&b, b"b").unwrap();
    let _ga = Guard(a.clone());
    let _gb = Guard(b.clone());

    assert!(run(&["lock", "--no-elevate"], &[&a, &b]).status.success());
    let o = run(&["status"], &[&a, &b]);
    assert_eq!(stdout(&o), format!("locked\t{}\nlocked\t{}\n", a.display(), b.display()));
    assert!(run(&["unlock", "--no-elevate"], &[&a, &b]).status.success());
}

#[test]
fn unlock_of_inherited_child_fails_with_exit_1_and_message() {
    let (_d, _g, root, f) = tree();
    assert!(run(&["lock", "--no-elevate"], &[&root]).status.success());

    let o = run(&["unlock", "--no-elevate"], &[&f]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("locked by a parent folder"), "{err}");
}

#[test]
fn missing_path_exits_1_and_status_reports_error() {
    let dir = tempfile::tempdir().unwrap();
    let nope = dir.path().join("nope");
    let o = run(&["lock", "--no-elevate"], &[&nope]);
    assert_eq!(o.status.code(), Some(1));
    let o = run(&["status"], &[&nope]);
    assert_eq!(o.status.code(), Some(1));
    assert_eq!(stdout(&o), format!("error\t{}\n", nope.display()));
}

#[test]
fn usage_errors_exit_2() {
    let o = Command::new(EXE).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("usage:"));
    let o = Command::new(EXE).args(["lock", "--bogus", "x"]).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
}

/// A name holding an unpaired surrogate must reach the operation intact.
/// `std::env::args()` panics on such an argument, and a lossy conversion would
/// point the command at a different file.
#[test]
fn a_lone_surrogate_in_the_name_is_handled_not_mangled() {
    let dir = tempfile::tempdir().unwrap();
    let odd =
        dir.path().join(OsString::from_wide(&[b'a' as u16, 0xD800, b'.' as u16, b't' as u16]));
    fs::write(&odd, b"x").unwrap();
    let _g = Guard(odd.clone());

    let o = run(&["lock", "--no-elevate"], &[&odd]);
    assert!(
        o.status.success(),
        "exit {:?}: {}",
        o.status.code(),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(fs::write(&odd, b"y").is_err(), "the wrong file was locked");

    let o = run(&["status"], &[&odd]);
    assert!(o.status.success());
    assert!(stdout(&o).starts_with("locked	"), "{}", stdout(&o));

    let o = run(&["unlock", "--no-elevate"], &[&odd]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    fs::write(&odd, b"y").unwrap();
}
