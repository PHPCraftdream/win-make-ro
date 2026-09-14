//! Regressions for defects found in review at 5fae70c.

use std::fs;
use std::path::{Path, PathBuf};

use ro_core::{
    ErrorKind as RoKind, LOCK_MASK, LockState, lock, lock_state, lock_tree, unlock, unlock_tree,
    wide_path,
};
use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSecurityDescriptorToStringSecurityDescriptorW,
    ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW,
    GetNamedSecurityInfoW, SDDL_REVISION_1, SE_FILE_OBJECT, SetNamedSecurityInfoW,
};
use windows::Win32::Security::{
    ACE_HEADER, ACL, ACL_REVISION, AddAce, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION,
    GetLengthSid, GetSecurityDescriptorDacl, INHERIT_ONLY_ACE, InitializeAcl, OBJECT_INHERIT_ACE,
    PSECURITY_DESCRIPTOR, PSID,
};
use windows::core::{PCWSTR, PWSTR};

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

/// The DACL in SDDL form. Unlike icacls output this does not change with the
/// system language, so assertions on it hold on any Windows.
fn dacl_sddl(path: &Path) -> String {
    let wide = wide_path(path).unwrap();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: wide is NUL-terminated; sd receives a LocalAlloc'd descriptor.
    let rc = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            &mut sd,
        )
    };
    assert_eq!(rc, ERROR_SUCCESS, "read security descriptor");
    let mut text = PWSTR::null();
    // SAFETY: sd is the descriptor just read; text receives a LocalAlloc'd string.
    unsafe {
        ConvertSecurityDescriptorToStringSecurityDescriptorW(
            sd,
            SDDL_REVISION_1,
            DACL_SECURITY_INFORMATION,
            &mut text,
            None,
        )
    }
    .expect("descriptor to sddl");
    // SAFETY: text is a NUL-terminated string owned by us until freed below.
    let out = unsafe { text.to_string() }.expect("utf16");
    // SAFETY: both allocations came from the calls above.
    unsafe {
        LocalFree(Some(HLOCAL(text.0.cast())));
        LocalFree(Some(HLOCAL(sd.0)));
    }
    out
}

fn icacls_text(path: &Path) -> String {
    let out = std::process::Command::new("icacls").arg(path).output().expect("icacls");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn icacls(args: &[&std::ffi::OsStr]) {
    let out = std::process::Command::new("icacls").args(args).output().expect("icacls");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

/// Builds a deny ACE carrying exactly LOCK_MASK plus INHERIT_ONLY.
///
/// icacls expands its `(W)` shorthand to FILE_GENERIC_WRITE, and the runner's
/// PowerShell cannot autoload the module holding `Set-Acl`, so the ACE is
/// assembled by hand. Inherited entries come back on their own because the
/// DACL is written unprotected.
fn deny_inherit_only_lock_mask(path: &Path) {
    const DENY_TYPE: u8 = 1;
    let sid_text: Vec<u16> = "S-1-1-0".encode_utf16().chain(Some(0)).collect();
    let mut psid = PSID::default();
    // SAFETY: sid_text is NUL-terminated; psid receives a LocalAlloc'd SID.
    unsafe { ConvertStringSidToSidW(PCWSTR(sid_text.as_ptr()), &mut psid) }.expect("sid");
    // SAFETY: psid is a valid SID until freed below.
    let sid_len = unsafe { GetLengthSid(psid) } as usize;

    let ace_len = std::mem::size_of::<ACE_HEADER>() + 4 + sid_len;
    let words = (std::mem::size_of::<ACL>() + ace_len).div_ceil(4);
    let mut acl_buf = vec![0u32; words];
    let acl = acl_buf.as_mut_ptr() as *mut ACL;
    let mut ace = vec![0u8; ace_len];
    let flags = OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0 | INHERIT_ONLY_ACE.0;
    let header = ACE_HEADER { AceType: DENY_TYPE, AceFlags: flags as u8, AceSize: ace_len as u16 };
    // SAFETY: acl_buf is 4-byte aligned and words*4 long; ace is exactly ace_len.
    unsafe {
        InitializeAcl(acl, (words * 4) as u32, ACL_REVISION).expect("init acl");
        std::ptr::write_unaligned(ace.as_mut_ptr() as *mut ACE_HEADER, header);
        std::ptr::write_unaligned(ace.as_mut_ptr().add(4) as *mut u32, LOCK_MASK);
        std::ptr::copy_nonoverlapping(psid.0 as *const u8, ace.as_mut_ptr().add(8), sid_len);
        AddAce(acl, ACL_REVISION, u32::MAX, ace.as_ptr() as *const _, ace_len as u32)
            .expect("add ace");
    }

    let wide = wide_path(path).unwrap();
    // SAFETY: wide is NUL-terminated; acl is a valid, initialised ACL.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
    };
    // SAFETY: psid came from ConvertStringSidToSidW.
    unsafe { LocalFree(Some(HLOCAL(psid.0))) };
    assert_eq!(rc, ERROR_SUCCESS, "write inherit-only ACE");
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
    assert!(fs::write(&shadowed, b"y").is_err(), "shadowed child stayed writable");
    assert!(fs::write(&plain, b"y").is_err());
    // Two changes: the root, and the shadowed child that needed an ACE of its
    // own. A child with nothing shadowing the lock is left to inheritance.
    assert_eq!(rep.changed, 2, "root + the shadowed child only");

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.changed, 2, "root + the child's own ACE");
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

/// A NULL DACL switches the access check off; no ACL reproduces that, and no
/// later unlock could prove a reconstructed one came from a NULL DACL. The
/// item is refused and left exactly as it was.
#[test]
fn null_dacl_is_refused_rather_than_reshaped() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f.txt");
    fs::write(&f, b"x").unwrap();
    let _g = Guard(f.clone());
    set_null_dacl(&f);
    let before = icacls_text(&f);

    let err = lock(&f).unwrap_err();
    assert!(matches!(err.kind, RoKind::NullDacl), "{err}");
    assert_eq!(icacls_text(&f), before, "the DACL was touched");
    assert_eq!(fs::read(&f).unwrap(), b"x");
    fs::write(&f, b"y").unwrap();
    assert!(!fs::metadata(&f).unwrap().permissions().readonly(), "attribute left behind");
}

/// Same through the recursive path: the error is reported, nothing changes.
#[test]
fn null_dacl_directory_is_refused_by_the_tree_walk() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    set_null_dacl(&root);
    let before = icacls_text(&root);

    let rep = lock_tree(&root);
    assert_eq!(rep.changed, 0);
    assert_eq!(rep.errors.len(), 1);
    assert!(matches!(rep.errors[0].kind, RoKind::NullDacl), "{}", rep.errors[0]);
    assert_eq!(icacls_text(&root), before);
    fs::write(root.join("new.txt"), b"n").unwrap();
}

/// The converse: a deliberate `Everyone: FullControl` with no inheritance
/// flags looks exactly like a reconstructed NULL DACL would, and must come
/// back untouched. Replacing it with a NULL DACL would hand access to tokens
/// carrying `Everyone` as deny-only, which the ACE denies.
#[test]
fn explicit_full_control_survives_a_lock_unlock_cycle_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f.txt");
    fs::write(&f, b"x").unwrap();
    let _g = Guard(f.clone());
    icacls(&[f.as_os_str(), "/inheritance:r".as_ref()]);
    icacls(&[f.as_os_str(), "/grant".as_ref(), "*S-1-1-0:(F)".as_ref()]);
    let before = dacl_sddl(&f);
    assert!(before.contains("(A;;FA;;;WD)"), "{before}");

    assert!(lock(&f).unwrap());
    assert!(fs::write(&f, b"y").is_err());
    assert!(unlock(&f).unwrap());

    let after = dacl_sddl(&f);
    // A NULL DACL is spelled NO_ACCESS_CONTROL in SDDL.
    assert!(!after.contains("NO_ACCESS_CONTROL"), "turned into a NULL DACL: {after}");
    assert_eq!(after, before, "the ACL did not come back unchanged");
    fs::write(&f, b"y").unwrap();
}

/// While a parent's lock applies the item reports as parent-locked, because
/// that is the only removal that can work. The menu relies on this to avoid
/// offering an unlock that would be refused.
#[test]
fn an_item_locked_by_both_reports_as_parent_locked() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    let f = root.join("f.txt");
    fs::write(&f, b"x").unwrap();

    assert!(lock(&f).unwrap());
    assert_eq!(lock_state(&f).unwrap(), LockState::Explicit);
    assert!(lock(&root).unwrap());
    assert_eq!(lock_state(&f).unwrap(), LockState::Inherited, "own ACE is not removable now");
    assert!(matches!(unlock(&f).unwrap_err().kind, RoKind::LockedByParent));

    assert!(unlock(&root).unwrap());
    assert_eq!(lock_state(&f).unwrap(), LockState::Explicit, "removable again");
    assert!(unlock(&f).unwrap());
}

/// The materialised NULL-DACL allow carries no inheritance flags, so an
/// ordinary inheritable `Everyone:(F)` must not be mistaken for it: restoring
/// a NULL DACL there strips the children of every permission they inherit.
#[test]
fn inheritable_full_control_is_not_mistaken_for_a_null_dacl() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    icacls(&[root.as_os_str(), "/inheritance:r".as_ref()]);
    icacls(&[root.as_os_str(), "/grant".as_ref(), "*S-1-1-0:(OI)(CI)(F)".as_ref()]);
    let f = root.join("f.txt");
    fs::write(&f, b"x").unwrap();

    assert!(lock(&root).unwrap());
    assert!(unlock(&root).unwrap());

    assert_eq!(fs::read(&f).unwrap(), b"x", "child lost its inherited permissions");
    fs::write(&f, b"y").unwrap();
    let sddl = dacl_sddl(&root);
    assert!(sddl.contains("(A;OICI;FA;;;WD)"), "{sddl}");
}

/// Under a locked parent the inherited deny already blocks WRITE_ATTRIBUTES,
/// so setting the READONLY attribute fails. That must not stop the child from
/// getting the deny ACE it needs.
#[test]
fn child_is_locked_even_when_the_readonly_attribute_cannot_be_set() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    let f = root.join("f.txt");
    fs::write(&f, b"x").unwrap();
    // Write data only: narrower than (W), and enough to shadow the lock.
    icacls(&[f.as_os_str(), "/grant".as_ref(), "*S-1-1-0:(WD)".as_ref()]);

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    // The explicit allow would otherwise win, so a write failing proves the
    // child got a deny ACE of its own despite the attribute refusing to move.
    assert!(fs::write(&f, b"y").is_err(), "child stayed writable");

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert_eq!(rep.changed, 2, "root + the child's own ACE");
    fs::write(&f, b"y").unwrap();
}

/// Unlocking a child whose parent is locked cannot make it writable, and used
/// to strip the ACE while leaving the READONLY attribute stuck for good.
#[test]
fn unlocking_a_child_under_a_locked_parent_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    let f = root.join("f.txt");
    fs::write(&f, b"x").unwrap();

    assert!(lock(&f).unwrap());
    assert!(lock(&root).unwrap());
    let err = unlock(&f).unwrap_err();
    assert!(matches!(err.kind, RoKind::LockedByParent), "{err}");
    assert_eq!(lock_state(&f).unwrap(), LockState::Inherited, "the parent governs now");

    assert!(unlock(&root).unwrap());
    // Returning true proves the child's own ACE survived the refused unlock.
    assert!(unlock(&f).unwrap());
    assert!(!fs::metadata(&f).unwrap().permissions().readonly(), "READONLY stayed behind");
    fs::write(&f, b"y").unwrap();
}

/// An INHERIT_ONLY ACE does not apply to the object carrying it, so a folder
/// holding one is not locked and must not be reported as such.
#[test]
fn inherit_only_ace_does_not_count_as_a_lock() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    deny_inherit_only_lock_mask(&root);

    assert_eq!(lock_state(&root).unwrap(), LockState::Unlocked, "IO ACE is not a lock");
    fs::create_dir(root.join("sub")).expect("creation is not actually blocked");
    // Locking still works and adds an ACE that does apply to the folder.
    assert!(lock(&root).unwrap());
    assert_eq!(lock_state(&root).unwrap(), LockState::Explicit);
    assert!(fs::create_dir(root.join("sub2")).is_err());
    assert!(unlock(&root).unwrap());
}

/// Applies `sddl` (a DACL-only descriptor) to `path`. Inheritance is left on,
/// so the entries a parent propagates still arrive alongside it.
fn set_dacl_from_sddl(path: &Path, sddl: &str) {
    let wide_sddl: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: wide_sddl is NUL-terminated; sd receives a LocalAlloc'd descriptor.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(wide_sddl.as_ptr()),
            SDDL_REVISION_1,
            &mut sd,
            None,
        )
    }
    .expect("parse sddl");
    let mut present = windows::core::BOOL(0);
    let mut defaulted = windows::core::BOOL(0);
    let mut acl: *mut ACL = std::ptr::null_mut();
    // SAFETY: sd is a valid descriptor from the call above.
    unsafe { GetSecurityDescriptorDacl(sd, &mut present, &mut acl, &mut defaulted) }
        .expect("read dacl");
    assert!(present.as_bool(), "the sddl carries no DACL");
    let wide = wide_path(path).unwrap();
    // SAFETY: wide is NUL-terminated; acl belongs to the live descriptor.
    let rc = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR(wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(acl),
            None,
        )
    };
    // SAFETY: sd came from ConvertStringSecurityDescriptorToSecurityDescriptorW.
    unsafe { LocalFree(Some(HLOCAL(sd.0))) };
    assert_eq!(rc, ERROR_SUCCESS, "apply sddl");
}

/// A conditional allow (SDDL `XA`, ACE type 9) grants exactly like a plain one
/// and is evaluated before an inherited deny, so treating only type 0 as
/// "allow" left the child writable while the tool called it locked.
#[test]
fn a_conditional_allow_on_a_child_is_not_mistaken_for_harmless() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    fs::create_dir(&root).unwrap();
    let _g = Guard(root.clone());
    let f = root.join("conditional.txt");
    fs::write(&f, b"x").unwrap();
    // Everyone, full access, guarded by a condition that always holds.
    set_dacl_from_sddl(&f, "D:(XA;;FA;;;WD;(Member_of {SID(S-1-1-0)}))");

    let rep = lock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    assert!(fs::write(&f, b"y").is_err(), "the conditional allow kept the child writable");

    let rep = unlock_tree(&root);
    assert!(rep.errors.is_empty(), "{:?}", rep.errors);
    fs::write(&f, b"z").unwrap();
}
