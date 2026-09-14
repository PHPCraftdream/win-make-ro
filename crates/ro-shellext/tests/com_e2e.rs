//! End-to-end through the real COM surface: the menu a selection produces,
//! the commands it runs, and the unload contract.

mod common;

use std::fs;

use ro_core::{LockState, lock_state};
use windows::Win32::Foundation::{S_FALSE, S_OK};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Shell::{CMINVOKECOMMANDINFO, GCS_VERBW, IShellExtInit};
use windows::core::PSTR;

use common::{COM_TESTS, Dll, Guard, context_menu, invoke, query, query_range, verb};

#[test]
fn full_cycle_through_com() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    // SAFETY: test-only switch read by the DLL; set before any COM object exists.
    unsafe { std::env::set_var("WIN_MAKE_RO_SYNC", "1") };

    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("root");
    let f = root.join("f.txt");
    fs::create_dir(&root).unwrap();
    fs::write(&f, b"x").unwrap();
    let _g = Guard(root.clone());

    let dll = Dll::load();
    assert_eq!(dll.can_unload(), S_OK, "no objects yet");

    // Unlocked folder: only "Make read only".
    let ctx = context_menu(&dll, &[&root]);
    assert_eq!(dll.can_unload(), S_FALSE, "object alive");
    assert_eq!(query(&ctx), vec![("Make read only".to_string(), true)]);
    assert_eq!(verb(&ctx, 0), "makereadonly");
    invoke(&ctx, 0);
    assert_eq!(lock_state(&root).unwrap(), LockState::Explicit);
    assert_eq!(lock_state(&f).unwrap(), LockState::Inherited);
    assert!(fs::write(&f, b"y").is_err());
    drop(ctx);
    assert_eq!(dll.can_unload(), S_OK, "object released");

    // Child locked via parent: disabled info item only, invoking it is refused.
    let ctx = context_menu(&dll, &[&f]);
    assert_eq!(query(&ctx), vec![("Read only (inherited from parent folder)".to_string(), false)]);
    let info = CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: fully initialized struct, offset 0.
    assert!(unsafe { ctx.InvokeCommand(&info) }.is_err());
    drop(ctx);

    // Locked folder + unlocked sibling file: both items, in order.
    let other = dir.path().join("other.txt");
    fs::write(&other, b"o").unwrap();
    let ctx = context_menu(&dll, &[&root, &other]);
    assert_eq!(
        query(&ctx),
        vec![("Make read only".to_string(), true), ("Remove read only".to_string(), true)]
    );
    assert_eq!(verb(&ctx, 1), "removereadonly");
    invoke(&ctx, 1);
    assert_eq!(lock_state(&root).unwrap(), LockState::Unlocked);
    assert_eq!(lock_state(&f).unwrap(), LockState::Unlocked);
    fs::write(&f, b"y").unwrap();
    drop(ctx);
    assert_eq!(dll.can_unload(), S_OK);
}

/// The class factory is a COM object the caller holds, and LockServer must
/// keep the DLL loaded on its own. Both used to report "safe to unload".
#[test]
fn unload_is_refused_while_the_factory_or_a_server_lock_is_held() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dll = Dll::load();
    assert_eq!(dll.can_unload(), S_OK, "nothing held yet");

    let factory = dll.factory();
    assert_eq!(dll.can_unload(), S_FALSE, "factory is still alive");
    drop(factory);
    assert_eq!(dll.can_unload(), S_OK, "factory released");

    let factory = dll.factory();
    // SAFETY: live factory.
    unsafe { factory.LockServer(true) }.unwrap();
    drop(factory);
    assert_eq!(dll.can_unload(), S_FALSE, "server lock outlives the factory");

    let factory = dll.factory();
    // SAFETY: live factory.
    unsafe { factory.LockServer(false) }.unwrap();
    drop(factory);
    assert_eq!(dll.can_unload(), S_OK, "lock released");
}

/// The handler must stay inside [idCmdFirst, idCmdLast] and must not write
/// through a zero-sized GetCommandString buffer.
#[test]
fn menu_honours_the_id_range_and_the_buffer_size() {
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
    let _g = Guard(locked.clone());

    let dll = Dll::load();
    let ctx = context_menu(&dll, &[&locked]);
    assert_eq!(query(&ctx), vec![("Make read only".to_string(), true)]);
    invoke(&ctx, 0);
    drop(ctx);

    // Mixed selection wants two items, but only one id is available.
    let ctx = context_menu(&dll, &[&locked, &free]);
    assert_eq!(query(&ctx).len(), 2, "both items fit in a wide range");
    let items = query_range(&ctx, 1000, 1000);
    assert_eq!(items.len(), 1, "only one id was offered");
    assert_eq!(items[0].2, 1000, "id outside [first, last]");
    let none = query_range(&ctx, 1000, 999);
    assert!(none.is_empty(), "no room at all");

    // A zero-sized buffer must be left untouched.
    let items = query_range(&ctx, 1000, 1001);
    assert_eq!(items.len(), 2);
    let mut buf = [0x1234u16; 8];
    // SAFETY: pszname points at buf; cchmax = 0 is the case under test.
    let _ =
        unsafe { ctx.GetCommandString(0, GCS_VERBW, None, PSTR(buf.as_mut_ptr() as *mut u8), 0) };
    assert_eq!(buf, [0x1234u16; 8], "wrote through a zero-sized buffer");
    drop(ctx);
}

#[test]
fn initialize_without_selection_fails() {
    let _serial = COM_TESTS.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: first COM call on this thread.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dll = Dll::load();
    let init: IShellExtInit = unsafe { dll.factory().CreateInstance(None) }.unwrap();
    // SAFETY: null data object is the case under test.
    let r = unsafe { init.Initialize(None, None, None) };
    assert!(r.is_err());
}
