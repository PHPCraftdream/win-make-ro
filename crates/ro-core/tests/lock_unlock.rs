//! Exercises real NTFS ACLs in a temp directory.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use ro_core::{ErrorKind as RoKind, LockState, lock, lock_state, lock_tree, unlock, unlock_tree};

/// Unlocks the whole tree on drop so the temp dir can be removed.
struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = unlock_tree(&self.0);
    }
}

fn sandbox() -> (tempfile::TempDir, Guard) {
    let dir = tempfile::tempdir().expect("temp dir");
    let guard = Guard(dir.path().to_path_buf());
    (dir, guard)
}

#[track_caller]
fn assert_denied<T: std::fmt::Debug>(r: std::io::Result<T>) {
    match r {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {}
        other => panic!("expected PermissionDenied, got {other:?}"),
    }
}

/// `del /F` clears READONLY first, which our ACE denies; Explorer does the same.
fn cmd_del_denied(p: &Path) {
    let out = std::process::Command::new("cmd")
        .args(["/C", "del", "/F", "/Q"])
        .arg(p)
        .output()
        .expect("cmd");
    assert!(p.exists(), "del must not remove {}", p.display());
    assert!(!out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

fn state(p: &Path) -> LockState {
    lock_state(p).expect("lock_state")
}

#[test]
fn file_lock_blocks_write_and_delete_then_unlock_restores() {
    let (dir, _g) = sandbox();
    let f = dir.path().join("a.txt");
    fs::write(&f, b"x").unwrap();
    assert_eq!(state(&f), LockState::Unlocked);

    assert!(lock(&f).unwrap());
    assert_eq!(state(&f), LockState::Explicit);
    assert!(fs::metadata(&f).unwrap().permissions().readonly());
    assert_denied(fs::write(&f, b"y"));
    cmd_del_denied(&f);
    assert_eq!(fs::read(&f).unwrap(), b"x", "reading stays allowed");

    assert!(unlock(&f).unwrap());
    assert_eq!(state(&f), LockState::Unlocked);
    fs::write(&f, b"y").unwrap();
    fs::remove_file(&f).unwrap();
}

/// Delete is granted through FILE_DELETE_CHILD on the parent even when the file
/// denies DELETE (CI runners hit this). The READONLY attribute makes shell
/// tools refuse, and our ACE keeps anyone from clearing the attribute. Rename
/// and POSIX-semantics delete still pass; only a folder lock stops those.
#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn file_lock_survives_delete_child_right_on_parent() {
    let (dir, _g) = sandbox();
    let out = std::process::Command::new("icacls")
        .arg(dir.path())
        .args(["/grant", "*S-1-1-0:(OI)(CI)(DC,WD)"])
        .output()
        .expect("icacls");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let f = dir.path().join("a.txt");
    fs::write(&f, b"x").unwrap();

    assert!(lock(&f).unwrap());
    assert!(fs::metadata(&f).unwrap().permissions().readonly());
    assert_denied(fs::write(&f, b"y"));
    cmd_del_denied(&f);
    // Clearing the attribute is denied too (FILE_WRITE_ATTRIBUTES).
    let mut p = fs::metadata(&f).unwrap().permissions();
    p.set_readonly(false);
    assert_denied(fs::set_permissions(&f, p));

    assert!(unlock(&f).unwrap());
    assert!(!fs::metadata(&f).unwrap().permissions().readonly());
    fs::remove_file(&f).unwrap();
}

#[test]
fn lock_is_idempotent_and_unlock_reports_noop() {
    let (dir, _g) = sandbox();
    let f = dir.path().join("a.txt");
    fs::write(&f, b"x").unwrap();
    assert!(lock(&f).unwrap());
    assert!(!lock(&f).unwrap(), "second lock is a no-op");
    assert!(unlock(&f).unwrap());
    assert!(!unlock(&f).unwrap(), "second unlock is a no-op");
    fs::write(&f, b"y").unwrap();
}

#[test]
fn dir_lock_propagates_to_children_and_blocks_creation() {
    let (dir, _g) = sandbox();
    let d = dir.path().join("d");
    let sub = d.join("sub");
    fs::create_dir_all(&sub).unwrap();
    let f = sub.join("f.txt");
    fs::write(&f, b"x").unwrap();

    assert!(lock(&d).unwrap());
    assert_eq!(state(&d), LockState::Explicit);
    assert_eq!(state(&sub), LockState::Inherited);
    assert_eq!(state(&f), LockState::Inherited);
    assert_denied(fs::write(d.join("new.txt"), b"n"));
    assert_denied(fs::create_dir(d.join("newdir")));
    assert_denied(fs::write(&f, b"y"));
    assert_denied(fs::remove_dir_all(&sub));

    assert!(unlock(&d).unwrap());
    assert_eq!(state(&sub), LockState::Unlocked);
    assert_eq!(state(&f), LockState::Unlocked);
    fs::write(&f, b"y").unwrap();
    fs::remove_dir_all(&d).unwrap();
}

#[test]
fn unlock_inherited_only_item_fails_with_locked_by_parent() {
    let (dir, _g) = sandbox();
    let d = dir.path().join("d");
    fs::create_dir(&d).unwrap();
    let f = d.join("f.txt");
    fs::write(&f, b"x").unwrap();
    lock(&d).unwrap();

    let err = unlock(&f).unwrap_err();
    assert!(matches!(err.kind, RoKind::LockedByParent), "{err}");
    assert_eq!(err.path, f);

    let rep = unlock_tree(&f);
    assert_eq!(rep.changed, 0);
    assert!(matches!(rep.errors[0].kind, RoKind::LockedByParent));
}

#[test]
fn unlock_tree_removes_nested_explicit_locks() {
    let (dir, _g) = sandbox();
    let root = dir.path().join("root");
    let inner = root.join("inner");
    fs::create_dir_all(&inner).unwrap();
    let f1 = root.join("f1.txt");
    let f2 = inner.join("f2.txt");
    fs::write(&f1, b"1").unwrap();
    fs::write(&f2, b"2").unwrap();

    // Nested explicit locks set before the root lock.
    lock(&f2).unwrap();
    lock(&inner).unwrap();
    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.changed, 1, "only root needed a change");
    // The nested ACEs are still there, but the root's lock is what governs
    // removal now, so every descendant reports as parent-locked.
    for p in [&f1, &f2, &inner] {
        assert_eq!(state(p), LockState::Inherited, "{}", p.display());
    }

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.changed, 3, "root + inner + f2");
    for p in [&root, &inner, &f1, &f2] {
        assert_eq!(state(p), LockState::Unlocked, "{}", p.display());
    }
    fs::write(&f2, b"z").unwrap();
}

#[test]
fn lock_tree_covers_children_with_blocked_inheritance() {
    let (dir, _g) = sandbox();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let f = root.join("protected.txt");
    fs::write(&f, b"x").unwrap();
    // Disable inheritance on the file (copy current ACL as explicit).
    let out = std::process::Command::new("icacls")
        .arg(&f)
        .args(["/inheritance:d"])
        .output()
        .expect("icacls");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.changed, 2, "root + explicitly locked protected file");
    assert_eq!(state(&f), LockState::Explicit);
    assert_denied(fs::write(&f, b"y"));

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(state(&f), LockState::Unlocked);
    fs::write(&f, b"y").unwrap();
}

#[test]
fn symlink_root_is_refused_and_junctions_are_skipped_in_walk() {
    let (dir, _g) = sandbox();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("t.txt"), b"t").unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let link = root.join("link");
    // mklink /J needs no privilege.
    let out = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(&target)
        .output()
        .expect("mklink");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));

    let err = lock(&link).unwrap_err();
    assert!(matches!(err.kind, RoKind::ReparsePoint), "{err}");

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.skipped, 1);
    assert_eq!(state(&target), LockState::Unlocked, "target must stay untouched");
    assert_eq!(state(&target.join("t.txt")), LockState::Unlocked);
    unlock_tree(&root);
}

#[test]
fn missing_path_reports_os_error() {
    let (dir, _g) = sandbox();
    let err = lock(&dir.path().join("nope")).unwrap_err();
    assert!(matches!(err.kind, RoKind::Os(_)));
    assert!(!err.is_access_denied());
}

#[test]
fn foreign_deny_ace_is_not_mistaken_for_our_lock() {
    let (dir, _g) = sandbox();
    let f = dir.path().join("a.txt");
    fs::write(&f, b"x").unwrap();
    // Deny only "write data" to Everyone: different mask, not ours.
    let out = std::process::Command::new("icacls")
        .arg(&f)
        .args(["/deny", "*S-1-1-0:(WD)"])
        .output()
        .expect("icacls");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    assert_eq!(state(&f), LockState::Unlocked);
    assert!(!unlock(&f).unwrap());
    // Our lock coexists with the foreign ACE and removal keeps the foreign one.
    assert!(lock(&f).unwrap());
    assert_eq!(state(&f), LockState::Explicit);
    assert!(unlock(&f).unwrap());
    assert_denied(fs::write(&f, b"y"));
    let out = std::process::Command::new("icacls")
        .arg(&f)
        .args(["/remove:d", "*S-1-1-0"])
        .output()
        .expect("icacls");
    assert!(out.status.success());
}
