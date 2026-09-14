//! Regressions found in review, driven through the real COM surface.

mod common;

use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use ro_core::{LockState, lock_state};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Threading::{GR_GDIOBJECTS, GetCurrentProcess, GetGuiResources};
use windows::Win32::UI::Shell::{CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, SEE_MASK_UNICODE};
use windows::core::{PCSTR, PCWSTR};

use common::{COM_TESTS, Dll, Guard, context_menu, invoke, query};

/// A file name may hold an unpaired surrogate. Folding it into U+FFFD points
/// the handler at a different path, so the menu described the wrong file.
#[test]
fn a_lone_surrogate_in_the_name_still_identifies_the_right_file() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let dir = tempfile::tempdir().unwrap();
    let odd =
        dir.path().join(OsString::from_wide(&[b'a' as u16, 0xD800, b'.' as u16, b't' as u16]));
    fs::write(&odd, b"x").unwrap();
    let _g = Guard(odd.clone());
    ro_core::lock(&odd).unwrap();
    assert_eq!(lock_state(&odd).unwrap(), LockState::Explicit);

    let dll = Dll::load();
    let ctx = context_menu(&dll, &[&odd]);
    assert_eq!(
        query(&ctx),
        vec![("Remove read only".to_string(), true)],
        "the handler read a different path"
    );
    drop(ctx);
}

/// With CMIC_MASK_UNICODE the verb lives in lpVerbW; reading lpVerb instead
/// used to run the first menu item, turning an unlock into a lock.
#[test]
fn a_unicode_verb_selects_the_command_it_names() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // SAFETY: test-only switch read by the DLL; set before any COM object exists.
    unsafe { std::env::set_var("WIN_MAKE_RO_SYNC", "1") };

    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked.txt");
    let free = dir.path().join("free.txt");
    fs::write(&locked, b"l").unwrap();
    fs::write(&free, b"f").unwrap();
    let _gl = Guard(locked.clone());
    let _gf = Guard(free.clone());
    ro_core::lock(&locked).unwrap();

    let dll = Dll::load();
    let ctx = context_menu(&dll, &[&locked, &free]);
    // Mixed selection: item 0 locks, item 1 unlocks.
    assert_eq!(
        query(&ctx),
        vec![("Make read only".to_string(), true), ("Remove read only".to_string(), true)]
    );

    let verb: Vec<u16> = "removereadonly".encode_utf16().chain(Some(0)).collect();
    let info = CMINVOKECOMMANDINFOEX {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
        fMask: SEE_MASK_UNICODE,
        lpVerbW: PCWSTR(verb.as_ptr()),
        ..Default::default()
    };
    // SAFETY: the struct is fully initialised and matches the advertised mask.
    unsafe { ctx.InvokeCommand(std::ptr::from_ref(&info).cast::<CMINVOKECOMMANDINFO>()) }
        .expect("InvokeCommand");

    assert_eq!(lock_state(&locked).unwrap(), LockState::Unlocked, "unlock did not run");
    assert_eq!(lock_state(&free).unwrap(), LockState::Unlocked, "the unlocked file got locked");
    drop(ctx);
}

/// The menu bitmap belongs to the object that built the menu. A process-wide
/// cache is never freed, so every load of the DLL left one more GDI object
/// behind; this drives whole load/unload cycles to see that.
#[test]
fn the_menu_bitmap_is_released_with_the_object() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("f.txt");
    fs::write(&f, b"x").unwrap();

    let gdi = || {
        // SAFETY: a pseudo-handle to our own process needs no release.
        unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) }
    };
    let round = |paths: &[&Path]| {
        let dll = Dll::load();
        let ctx = context_menu(&dll, paths);
        query(&ctx);
        drop(ctx);
        // Dropping the handle releases the module, so each round is a whole
        // load/unload cycle.
        drop(dll);
    };

    // Warm-up: the first menu also pulls in GDI state that is not ours.
    round(&[&f]);
    round(&[&f]);

    let rounds = 8;
    let before = gdi();
    for _ in 0..rounds {
        round(&[&f]);
    }
    let after = gdi();
    assert!(
        after < before + rounds,
        "GDI objects grew from {before} to {after} over {rounds} load/unload cycles"
    );
}

/// The numeric command id always travels in lpVerb, even when the caller sets
/// CMIC_MASK_UNICODE and leaves lpVerbW NULL. Reading the id from lpVerbW ran
/// the first menu item, turning a requested unlock into a lock.
#[test]
fn a_numeric_id_is_read_from_lp_verb_even_under_the_unicode_mask() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // SAFETY: test-only switch read by the DLL; set before any COM object exists.
    unsafe { std::env::set_var("WIN_MAKE_RO_SYNC", "1") };

    let dir = tempfile::tempdir().unwrap();
    let locked = dir.path().join("locked.txt");
    let free = dir.path().join("free.txt");
    fs::write(&locked, b"l").unwrap();
    fs::write(&free, b"f").unwrap();
    let _gl = Guard(locked.clone());
    let _gf = Guard(free.clone());
    ro_core::lock(&locked).unwrap();

    let dll = Dll::load();
    let ctx = context_menu(&dll, &[&locked, &free]);
    // Item 0 locks, item 1 unlocks.
    assert_eq!(
        query(&ctx),
        vec![("Make read only".to_string(), true), ("Remove read only".to_string(), true)]
    );

    let info = CMINVOKECOMMANDINFOEX {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFOEX>() as u32,
        fMask: SEE_MASK_UNICODE,
        // MAKEINTRESOURCE(1): a command offset, not a pointer.
        lpVerb: PCSTR(std::ptr::without_provenance(1)),
        lpVerbW: PCWSTR(std::ptr::null()),
        ..Default::default()
    };
    // SAFETY: the struct is fully initialised and matches the advertised mask.
    unsafe { ctx.InvokeCommand(std::ptr::from_ref(&info).cast::<CMINVOKECOMMANDINFO>()) }
        .expect("InvokeCommand");

    assert_eq!(lock_state(&locked).unwrap(), LockState::Unlocked, "item 1 did not run");
    assert_eq!(lock_state(&free).unwrap(), LockState::Unlocked, "the unlocked file got locked");
    drop(ctx);
}

/// Explorer hands over the whole selection, and a few hundred long names do
/// not fit on a command line. The handler used to build one anyway and
/// InvokeCommand failed outright.
#[test]
fn a_selection_too_large_for_a_command_line_still_runs() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // SAFETY: test-only switch read by the DLL; set before any COM object exists.
    unsafe { std::env::set_var("WIN_MAKE_RO_SYNC", "1") };

    let dir = tempfile::tempdir().unwrap();
    let _g = Guard(dir.path().to_path_buf());
    let mut owned = Vec::new();
    for i in 0..200 {
        let p = dir.path().join(format!("{}-{i:03}.txt", "n".repeat(160)));
        fs::write(&p, b"x").unwrap();
        owned.push(p);
    }
    let paths: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    assert!(!ro_core::fits_command_line(64, &paths), "the fixture is not large enough");

    let dll = Dll::load();
    let ctx = context_menu(&dll, &paths);
    // Past the probe limit the state of each item is not read, so both
    // commands are offered and the one picked decides.
    assert_eq!(
        query(&ctx),
        vec![("Make read only".to_string(), true), ("Remove read only".to_string(), true)]
    );
    invoke(&ctx, 0);
    for p in &owned {
        assert!(fs::write(p, b"y").is_err(), "{} stayed writable", p.display());
    }
    invoke(&ctx, 1);
    fs::write(&owned[0], b"y").unwrap();
    fs::write(owned.last().unwrap(), b"y").unwrap();
    drop(ctx);
}
