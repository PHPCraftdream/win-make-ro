//! Regressions for defects found in review at 5fae70c.

use std::fs;
use std::path::{Path, PathBuf};

use ro_core::{LockState, lock, lock_state, lock_tree, unlock, unlock_tree, wide_path};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::Security::Authorization::{SE_FILE_OBJECT, SetNamedSecurityInfoW};
use windows::Win32::Security::DACL_SECURITY_INFORMATION;
use windows::core::PCWSTR;

struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = unlock_tree(&self.0);
        // A failed test may leave a lock behind; make the tempdir removable.
        let _ = std::process::Command::new("icacls")
            .arg(&self.0)
            .args(["/reset", "/t", "/c", "/q"])
            .output();
    }
}

fn icacls_text(path: &Path) -> String {
    let out = std::process::Command::new("icacls").arg(path).output().expect("icacls");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn icacls(args: &[&std::ffi::OsStr]) {
    let out = std::process::Command::new("icacls").args(args).output().expect("icacls");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

fn junction(link: &Path, target: &Path) {
    let out = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .expect("mklink");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

fn set_null_dacl(path: &Path) {
    let wide = wide_path(path).unwrap();
    // SAFETY: wide is NUL-terminated; a None DACL pointer sets a NULL DACL.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
        )
    };
    assert_eq!(rc, ERROR_SUCCESS, "set NULL DACL");
}

/// An explicit allow on a child is evaluated before the inherited deny, so
/// relying on the inherited lock alone left the file writable.
#[test]
fn child_with_explicit_allow_gets_its_own_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    let shadowed = root.join("shadowed.txt");
    let plain = root.join("plain.txt");
    fs::write(&shadowed, b"x").unwrap();
    fs::write(&plain, b"x").unwrap();
    icacls(&[shadowed.as_os_str(), "/grant".as_ref(), "*S-1-1-0:(W)".as_ref()]);

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(lock_state(&shadowed).unwrap(), LockState::Explicit, "must not rely on inheritance");
    assert!(fs::write(&shadowed, b"y").is_err(), "shadowed child stayed writable");
    // A child with nothing shadowing the lock still needs no ACE of its own.
    assert_eq!(lock_state(&plain).unwrap(), LockState::Inherited);
    assert!(fs::write(&plain, b"y").is_err());
    assert_eq!(rep.changed, 2, "root + the shadowed child only");

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(lock_state(&shadowed).unwrap(), LockState::Unlocked);
    fs::write(&shadowed, b"y").unwrap();
    fs::write(&plain, b"y").unwrap();
}

/// Both tree operations used to report the root as skipped and then walk
/// through it anyway, rewriting ACLs inside the junction's target.
#[test]
fn tree_ops_on_a_junction_root_leave_the_target_alone() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    let inside = target.join("t.txt");
    fs::write(&inside, b"t").unwrap();
    let link = dir.path().join("link");
    junction(&link, &target);
    let _g = Guard(target.clone());

    let rep = lock_tree(&link);
    assert_eq!(rep.changed, 0);
    assert_eq!(rep.skipped, 1);
    assert_eq!(rep.errors.len(), 1);
    assert_eq!(lock_state(&inside).unwrap(), LockState::Unlocked, "target was modified");
    assert_eq!(lock_state(&target).unwrap(), LockState::Unlocked);
    fs::write(&inside, b"still writable").unwrap();

    // Same for unlock: lock the target directly, then try to unlock via the link.
    lock(&target).unwrap();
    let rep = unlock_tree(&link);
    assert_eq!(rep.changed, 0);
    assert_eq!(rep.skipped, 1);
    assert_eq!(lock_state(&target).unwrap(), LockState::Explicit, "target was unlocked via link");
    unlock(&target).unwrap();
}

/// A NULL DACL allows everyone everything; an ACL holding only our deny allows
/// nobody anything. The lock used to destroy access, and unlock could not even
/// read the file back to repair it.
#[test]
fn null_dacl_survives_a_lock_unlock_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f.txt");
    fs::write(&f, b"x").unwrap();
    let _g = Guard(f.clone());
    set_null_dacl(&f);

    assert!(lock(&f).unwrap());
    assert_eq!(lock_state(&f).unwrap(), LockState::Explicit);
    assert_eq!(fs::read(&f).unwrap(), b"x", "reading must survive the lock");
    assert!(fs::write(&f, b"y").is_err(), "writing must be denied");

    assert!(unlock(&f).unwrap());
    assert_eq!(lock_state(&f).unwrap(), LockState::Unlocked);
    assert_eq!(fs::read(&f).unwrap(), b"x");
    fs::write(&f, b"y").unwrap();
    // Restored as a NULL DACL, not as an empty one: icacls spells the former
    // "No permissions are set. All users have full control." and prints an
    // empty ACE list for the latter.
    assert!(icacls_text(&f).contains("All users have full control"), "{}", icacls_text(&f));
}

/// Same for a directory, through the recursive path. Note that Windows itself
/// strips inherited ACEs from the children when a directory is given a NULL
/// DACL, so only the directory's own access is under test here.
#[test]
fn null_dacl_directory_survives_a_lock_unlock_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    set_null_dacl(&root);
    assert!(fs::write(root.join("before.txt"), b"b").is_ok());

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(lock_state(&root).unwrap(), LockState::Explicit);
    assert!(fs::write(root.join("new.txt"), b"n").is_err(), "creation must be denied");

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(lock_state(&root).unwrap(), LockState::Unlocked);
    assert!(icacls_text(&root).contains("All users have full control"), "{}", icacls_text(&root));
    fs::write(root.join("new.txt"), b"n").unwrap();
}
