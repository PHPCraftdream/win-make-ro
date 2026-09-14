//! End-to-end: drives the built executable on a temp tree.

use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};
use windows::Win32::Security::{
    ACL, ACL_REVISION, AddAce, DACL_SECURITY_INFORMATION, InitializeAcl,
    PROTECTED_DACL_SECURITY_INFORMATION,
};
use windows::Win32::Storage::FileSystem::{FILE_ALL_ACCESS, FILE_GENERIC_READ};
use windows::core::PCWSTR;

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

/// Windows lets the same item be spelled several ways, and putting the strings
/// in order does not make those spellings meet. Naming a child before a parent
/// that was written differently used to be refused outright: both ended up
/// unlocked, but the run reported a failure that had not happened.
#[test]
fn overlapping_targets_survive_a_differently_spelled_parent() {
    for spelling in ["upper case", "verbatim prefix"] {
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let child = parent.join("child.txt");
        fs::write(&child, b"x").unwrap();
        let _g = Guard(parent.clone());
        assert!(run(&["lock", "--no-elevate"], &[&parent]).status.success());

        // Both spellings sort ahead of the plain parent, so the child is tried
        // first no matter which order the two are given in.
        let odd = match spelling {
            "upper case" => dir.path().join("PARENT").join("child.txt"),
            _ => PathBuf::from(format!(r"\\?\{}", child.display())),
        };
        let o = run(&["unlock", "--no-elevate"], &[&odd, &parent]);
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success(), "{spelling}: exit {:?}: {err}", o.status.code());
        assert!(err.is_empty(), "{spelling}: {err}");
        fs::write(&child, b"y").expect("the child stayed locked");
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

    // `--consume-paths-from` is how the shell extension hands its list over:
    // nothing there can wait for the helper, so the helper removes it.
    let list = ro_core::write_paths_file(&paths).unwrap();
    let o = Command::new(EXE)
        .args(["lock", "--no-elevate", "--consume-paths-from"])
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
        .args(["unlock", "--no-elevate", "--consume-paths-from"])
        .arg(&list)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    fs::write(&paths[0], b"y").unwrap();
}

/// A list named with `--paths-from` belongs to whoever wrote it. Reading it is
/// no reason to destroy it — least of all for `status`, which changes nothing,
/// and least of all when the run failed and the list is what a retry needs.
#[test]
fn a_path_list_the_caller_named_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.txt");
    fs::write(&target, b"x").unwrap();
    let missing = dir.path().join("missing.txt");

    for (paths, code) in [(vec![target.clone()], 0), (vec![missing.clone()], 1)] {
        let list = dir.path().join("mine.paths");
        let written = ro_core::write_paths_file(&paths).unwrap();
        fs::rename(&written, &list).unwrap();
        let before = fs::read(&list).unwrap();

        let o = Command::new(EXE).args(["status", "--paths-from"]).arg(&list).output().unwrap();
        assert_eq!(o.status.code(), Some(code), "{}", String::from_utf8_lossy(&o.stderr));
        assert!(list.is_file(), "the caller's list was deleted");
        assert_eq!(fs::read(&list).unwrap(), before, "the caller's list was rewritten");
        fs::remove_file(&list).unwrap();
    }

    // The same for a command that does change things.
    let list = dir.path().join("mine.paths");
    let written = ro_core::write_paths_file(std::slice::from_ref(&target)).unwrap();
    fs::rename(&written, &list).unwrap();
    let _g = Guard(target.clone());
    assert!(
        Command::new(EXE)
            .args(["lock", "--no-elevate", "--paths-from"])
            .arg(&list)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(list.is_file(), "the caller's list was deleted by lock");
    assert!(fs::write(&target, b"y").is_err(), "the list was read but nothing happened");
}

/// One allow ACE of the smallest possible shape: header, mask and a SID with a
/// single sub-authority, which is 20 bytes altogether.
fn allow_ace(authority: u8, sub_authority: u32, mask: u32) -> [u8; 20] {
    let mut ace = [0u8; 20];
    ace[0] = 0; // ACCESS_ALLOWED_ACE_TYPE
    ace[2..4].copy_from_slice(&20u16.to_le_bytes()); // AceSize
    ace[4..8].copy_from_slice(&mask.to_le_bytes());
    ace[8] = 1; // SID revision
    ace[9] = 1; // one sub-authority
    ace[15] = authority; // identifier authority, big-endian over six bytes
    ace[16..20].copy_from_slice(&sub_authority.to_le_bytes());
    ace
}

/// Fills the item's DACL to the 64 KB an ACL can hold: `Everyone: FullControl`
/// followed by distinct read-only entries until no further ACE fits.
///
/// The SIDs have to differ — Windows may merge identical entries, and a DACL
/// that quietly shrank would not exercise the limit at all. The DACL is written
/// protected so nothing is inherited on top of it.
fn fill_dacl_to_the_limit(path: &Path) {
    const ACL_HEADER: usize = std::mem::size_of::<ACL>();
    const ACE_BYTES: usize = 20;
    let count = (usize::from(u16::MAX) - ACL_HEADER) / ACE_BYTES;
    let bytes = ACL_HEADER + count * ACE_BYTES;

    let mut buf = vec![0u32; bytes / 4];
    let acl = buf.as_mut_ptr() as *mut ACL;
    // SAFETY: buf is 4-byte aligned and exactly `bytes` long.
    unsafe { InitializeAcl(acl, bytes as u32, ACL_REVISION) }.expect("init acl");
    let mut entries = vec![allow_ace(1, 0, FILE_ALL_ACCESS.0)]; // S-1-1-0
    entries.extend((1..count).map(|i| allow_ace(5, 10_000 + i as u32, FILE_GENERIC_READ.0)));
    for ace in &entries {
        // SAFETY: each ACE is a well-formed 20-byte ACCESS_ALLOWED_ACE and the
        // buffer was sized for exactly this many of them.
        unsafe { AddAce(acl, ACL_REVISION, u32::MAX, ace.as_ptr() as *const _, ACE_BYTES as u32) }
            .expect("add ace");
    }

    let wide = ro_core::wide_path(path).unwrap();
    // SAFETY: wide is NUL-terminated and acl is a valid, initialised ACL.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
    };
    assert_eq!(rc, ERROR_SUCCESS, "write a full-size DACL");
}

/// An ACL stores its own size in a `WORD`, so a DACL can reach 64 KB and no
/// further. One filled to the brim used to make the helper panic while sizing
/// the buffer for the lock ACE, which took the whole run down: every target
/// after it was left alone without a word.
#[test]
fn a_dacl_at_the_size_limit_is_reported_and_the_next_target_still_runs() {
    let dir = tempfile::tempdir().unwrap();
    // `ordered` sorts the targets, so the names decide which one comes first.
    let full = dir.path().join("a-full-dacl.txt");
    let next = dir.path().join("z-next.txt");
    fs::write(&full, b"x").unwrap();
    fs::write(&next, b"x").unwrap();
    let _g = Guard(next.clone());
    fill_dacl_to_the_limit(&full);

    let o = run(&["lock", "--no-elevate"], &[&full, &next]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(1), "exit code; stderr: {err}");
    assert!(err.contains("64 KB"), "{err}");
    // The ACE never went in, so the file is exactly as it was.
    fs::write(&full, b"y").expect("the item was changed after all");
    // And the target behind it was still processed.
    assert!(fs::write(&next, b"y").is_err(), "the run stopped at the first failure");
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
    // `--paths-from` leaves the file to its author, which here is this test.
    ro_core::remove_paths_file(&list);
}
