//! End-to-end through the real COM surface: loads the built DLL, feeds it a
//! shell data object, inspects the menu it builds and invokes the commands.

use std::ffi::OsStr;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ro_core::{LockState, lock_state, unlock_tree};
use windows::Win32::Foundation::{HMODULE, S_FALSE, S_OK};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, IClassFactory, IDataObject,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{
    BHID_DataObject, CMINVOKECOMMANDINFO, GCS_VERBW, IContextMenu, ILCreateFromPathW, ILFree,
    IShellExtInit, SHCreateShellItemArrayFromIDLists,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemInfoW, GetMenuState, GetMenuStringW,
    MENUITEMINFOW, MF_BYPOSITION, MF_GRAYED, MIIM_BITMAP,
};
use windows::core::{GUID, HRESULT, Interface, PCSTR, PCWSTR, PSTR};

const CLSID: GUID = GUID::from_u128(0x7A3C1F0E_5B2D_4E8A_9C61_0D4F2B7E9A11);
const ID_FIRST: u32 = 1000;

// Tests share the DLL's process-wide object and server-lock counts.
static COM_TESTS: Mutex<()> = Mutex::new(());

type GetClassObject =
    unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut core::ffi::c_void) -> HRESULT;
type NoArgs = unsafe extern "system" fn() -> HRESULT;

fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// target/debug from target/debug/deps/<test>.exe
fn target_dir() -> PathBuf {
    std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

/// `cargo test` alone does not emit the cdylib or the helper into target/;
/// build both so the DLL can be loaded and can find `win-make-ro.exe`.
fn ensure_built() {
    let st = std::process::Command::new(env!("CARGO"))
        .args(["build", "-p", "win-make-ro", "-p", "ro-shellext"])
        .status()
        .expect("cargo build");
    assert!(st.success());
    assert!(target_dir().join("win-make-ro.exe").is_file());
}

struct Dll {
    module: HMODULE,
}

impl Dll {
    fn load() -> Self {
        ensure_built();
        let path = target_dir().join("ro_shellext.dll");
        assert!(path.is_file(), "{} missing", path.display());
        let w = wide(path.as_os_str());
        // SAFETY: w is NUL-terminated.
        let module = unsafe { LoadLibraryW(PCWSTR(w.as_ptr())) }.expect("LoadLibrary");
        Self { module }
    }

    fn sym<T: Copy>(&self, name: &str) -> T {
        let c = format!("{name}\0");
        // SAFETY: c is NUL-terminated; the DLL exports these symbols with the given ABI.
        let p = unsafe { GetProcAddress(self.module, PCSTR(c.as_ptr())) }.expect("symbol");
        // SAFETY: caller names the export whose signature matches T.
        unsafe { std::mem::transmute_copy(&p) }
    }

    fn factory(&self) -> IClassFactory {
        let f: GetClassObject = self.sym("DllGetClassObject");
        let mut ppv: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: valid GUID pointers and out-pointer.
        unsafe { f(&CLSID, &IClassFactory::IID, &mut ppv) }.ok().expect("DllGetClassObject");
        // SAFETY: ppv holds an IClassFactory reference we now own.
        unsafe { IClassFactory::from_raw(ppv) }
    }

    fn can_unload(&self) -> HRESULT {
        let f: NoArgs = self.sym("DllCanUnloadNow");
        // SAFETY: no arguments.
        unsafe { f() }
    }
}

fn data_object(paths: &[&Path]) -> IDataObject {
    let pidls: Vec<_> = paths
        .iter()
        .map(|p| {
            let w = wide(p.as_os_str());
            // SAFETY: w is NUL-terminated; result freed below.
            unsafe { ILCreateFromPathW(PCWSTR(w.as_ptr())) as *const _ }
        })
        .collect();
    // SAFETY: pidls are valid absolute item id lists.
    let array = unsafe { SHCreateShellItemArrayFromIDLists(&pidls) }.expect("item array");
    for p in &pidls {
        // SAFETY: allocated by ILCreateFromPathW.
        unsafe { ILFree(Some(*p)) };
    }
    // SAFETY: BHID_DataObject is a valid handler id.
    unsafe { array.BindToHandler(None, &BHID_DataObject) }.expect("data object")
}

struct Menu(windows::Win32::UI::WindowsAndMessaging::HMENU);

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: handle from CreatePopupMenu.
        let _ = unsafe { DestroyMenu(self.0) };
    }
}

/// (label, enabled) per inserted item.
fn query(ctx: &IContextMenu) -> Vec<(String, bool)> {
    // SAFETY: plain menu creation.
    let menu = Menu(unsafe { CreatePopupMenu() }.unwrap());
    // SAFETY: menu is live; id range is ours.
    let hr = unsafe { ctx.QueryContextMenu(menu.0, 0, ID_FIRST, ID_FIRST + 100, 0) };
    assert!(hr.is_ok(), "{hr:?}");
    let n = hr.0 as usize;
    // SAFETY: menu is live.
    assert_eq!(unsafe { GetMenuItemCount(Some(menu.0)) } as usize, n);
    (0..n as u32)
        .map(|pos| {
            let mut buf = [0u16; 128];
            // SAFETY: buf is writable; pos < item count.
            let len =
                unsafe { GetMenuStringW(menu.0, pos, Some(&mut buf), MF_BYPOSITION) } as usize;
            let state = unsafe { GetMenuState(menu.0, pos, MF_BYPOSITION) };
            let mut mii = MENUITEMINFOW {
                cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_BITMAP,
                ..Default::default()
            };
            // SAFETY: mii is initialized; pos < item count.
            unsafe { GetMenuItemInfoW(menu.0, pos, true, &mut mii) }.unwrap();
            assert!(!mii.hbmpItem.0.is_null(), "every item carries the app icon bitmap");
            (String::from_utf16_lossy(&buf[..len]), state & MF_GRAYED.0 == 0)
        })
        .collect()
}

fn invoke(ctx: &IContextMenu, offset: usize) {
    let info = CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
        lpVerb: PCSTR(offset as *const u8),
        ..Default::default()
    };
    // SAFETY: info is fully initialized; lpVerb carries an offset (HIWORD == 0).
    unsafe { ctx.InvokeCommand(&info) }.expect("InvokeCommand");
}

fn verb(ctx: &IContextMenu, offset: usize) -> String {
    let mut buf = [0u16; 64];
    // SAFETY: GCS_VERBW writes UTF-16 into a buffer of cchmax chars.
    unsafe {
        ctx.GetCommandString(
            offset,
            GCS_VERBW,
            None,
            PSTR(buf.as_mut_ptr() as *mut u8),
            buf.len() as u32,
        )
    }
    .expect("GetCommandString");
    let end = buf.iter().position(|&c| c == 0).unwrap();
    String::from_utf16_lossy(&buf[..end])
}

fn context_menu(dll: &Dll, paths: &[&Path]) -> IContextMenu {
    let init: IShellExtInit =
        unsafe { dll.factory().CreateInstance(None) }.expect("CreateInstance");
    let data = data_object(paths);
    // SAFETY: data is a live data object; pidl/hkey may be null.
    unsafe { init.Initialize(None, &data, None) }.expect("Initialize");
    init.cast().expect("IContextMenu")
}

struct Guard(PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = unlock_tree(&self.0);
    }
}

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
