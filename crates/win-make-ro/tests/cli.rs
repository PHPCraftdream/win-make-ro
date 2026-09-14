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
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("usage:"));
    // npm hides what its scripts print, so this is where a user installed
    // through npm can still find the removal order.
    let unregister = err.find("win-make-ro uninstall").expect("no unregister hint");
    let remove = err.find("npm uninstall -g win-make-ro").expect("no npm hint");
    assert!(unregister < remove, "unregistering has to come first");
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

/// Overlapping targets used to give different results depending on the order
/// they were typed in: unlocking a child first was refused while its parent
/// was still locked, even though both ended up unlocked either way.
#[test]
fn overlapping_targets_behave_the_same_in_either_order() {
    for reversed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let child = parent.join("child.txt");
        fs::write(&child, b"x").unwrap();
        let _g = Guard(parent.clone());

        assert!(run(&["lock", "--no-elevate"], &[&parent]).status.success());
        let targets: Vec<&Path> =
            if reversed { vec![&child, &parent] } else { vec![&parent, &child] };
        let o = run(&["unlock", "--no-elevate"], &targets);
        assert!(
            o.status.success(),
            "reversed={reversed}: exit {:?}: {}",
            o.status.code(),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(String::from_utf8_lossy(&o.stderr).is_empty(), "reversed={reversed}");
        fs::write(&child, b"y").unwrap();
    }
}

/// A selection larger than a command line has to travel some other way. The
/// helper reads the list from a file and removes it afterwards.
#[test]
fn a_selection_too_large_for_a_command_line_goes_through_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let _g = Guard(dir.path().to_path_buf());
    let mut paths = Vec::new();
    for i in 0..200 {
        // Long enough that 200 of them cannot share one command line.
        let p = dir.path().join(format!("{}-{i:03}.txt", "n".repeat(160)));
        fs::write(&p, b"x").unwrap();
        paths.push(p);
    }
    assert!(!ro_core::fits_command_line(64, &paths), "the fixture is not large enough");

    let list = ro_core::write_paths_file(&paths).unwrap();
    let o = Command::new(EXE)
        .args(["lock", "--no-elevate", "--paths-from"])
        .arg(&list)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(!list.exists(), "the list was left behind");
    for p in &paths {
        assert!(fs::write(p, b"y").is_err(), "{} stayed writable", p.display());
    }

    let list = ro_core::write_paths_file(&paths).unwrap();
    let o = Command::new(EXE)
        .args(["unlock", "--no-elevate", "--paths-from"])
        .arg(&list)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    fs::write(&paths[0], b"y").unwrap();
}

/// The list is UTF-16, so a name that is not valid Unicode survives it.
#[test]
fn a_path_list_keeps_a_lone_surrogate() {
    let dir = tempfile::tempdir().unwrap();
    let odd =
        dir.path().join(OsString::from_wide(&[b'a' as u16, 0xD800, b'.' as u16, b't' as u16]));
    fs::write(&odd, b"x").unwrap();
    let _g = Guard(odd.clone());

    let list = ro_core::write_paths_file(std::slice::from_ref(&odd)).unwrap();
    assert_eq!(ro_core::read_paths_file(&list).unwrap(), vec![odd.clone()]);

    let o = Command::new(EXE)
        .args(["lock", "--no-elevate", "--paths-from"])
        .arg(&list)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(fs::write(&odd, b"y").is_err(), "the wrong file was locked");
}
