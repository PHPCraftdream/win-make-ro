//! Shared scaffolding for the COM end-to-end tests: loads the built DLL,
//! builds a shell data object, and reads back the menu it produces.
//!
//! Each test binary gets its own copy, which is deliberate: the DLL keeps
//! process-wide counters, so separate processes keep the tests independent.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ro_core::unlock_tree;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::Com::{IClassFactory, IDataObject};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{
    BHID_DataObject, CMINVOKECOMMANDINFO, GCS_VERBW, IContextMenu, ILCreateFromPathW, ILFree,
    IShellExtInit, SHCreateShellItemArrayFromIDLists,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemID, GetMenuItemInfoW, GetMenuState,
    GetMenuStringW, MENUITEMINFOW, MF_BYPOSITION, MF_GRAYED, MIIM_BITMAP,
};
use windows::core::{GUID, HRESULT, Interface, PCSTR, PCWSTR, PSTR};

pub const CLSID: GUID = GUID::from_u128(0x7A3C1F0E_5B2D_4E8A_9C61_0D4F2B7E9A11);
pub const ID_FIRST: u32 = 1000;

// Tests share the DLL's process-wide object and server-lock counts.
pub static COM_TESTS: Mutex<()> = Mutex::new(());

pub type GetClassObject =
    unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut core::ffi::c_void) -> HRESULT;
pub type NoArgs = unsafe extern "system" fn() -> HRESULT;

pub fn wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// target/<profile> from target/<profile>/deps/<test>.exe
pub fn target_dir() -> PathBuf {
    std::env::current_exe().unwrap().parent().unwrap().parent().unwrap().to_path_buf()
}

/// The profile this test itself was built with, taken from the artifact
/// directory rather than guessed, so a release run builds release artifacts.
pub fn profile() -> String {
    target_dir().file_name().unwrap().to_string_lossy().into_owned()
}

/// `cargo test` alone does not emit the cdylib or the helper into target/;
/// build both so the DLL can be loaded and can find `win-make-ro.exe`. The
/// build must target the same profile as this test, or a release run would
/// load whatever stale debug artifact happens to sit next to it.
pub fn ensure_built() {
    let profile = profile();
    let mut cmd = std::process::Command::new(env!("CARGO"));
    cmd.args(["build", "-p", "win-make-ro", "-p", "ro-shellext"]);
    cmd.arg("--target-dir").arg(target_dir().parent().expect("artifact root"));
    match profile.as_str() {
        "debug" => {}
        "release" => {
            cmd.arg("--release");
        }
        other => {
            cmd.args(["--profile", other]);
        }
    }
    let st = cmd.status().expect("cargo build");
    assert!(st.success(), "building the {profile} artifacts failed");
    assert!(
        target_dir().join("win-make-ro.exe").is_file(),
        "helper missing from the {profile} artifacts"
    );
}

pub struct Dll {
    pub module: HMODULE,
}

impl Dll {
    pub fn load() -> Self {
        ensure_built();
        let path = target_dir().join("ro_shellext.dll");
        assert!(path.is_file(), "{} missing", path.display());
        let w = wide(path.as_os_str());
        // SAFETY: w is NUL-terminated.
        let module = unsafe { LoadLibraryW(PCWSTR(w.as_ptr())) }.expect("LoadLibrary");
        Self { module }
    }

    pub fn sym<T: Copy>(&self, name: &str) -> T {
        let c = format!("{name}\0");
        // SAFETY: c is NUL-terminated; the DLL exports these symbols with the given ABI.
        let p = unsafe { GetProcAddress(self.module, PCSTR(c.as_ptr())) }.expect("symbol");
        // SAFETY: caller names the export whose signature matches T.
        unsafe { std::mem::transmute_copy(&p) }
    }

    pub fn factory(&self) -> IClassFactory {
        let f: GetClassObject = self.sym("DllGetClassObject");
        let mut ppv: *mut core::ffi::c_void = std::ptr::null_mut();
        // SAFETY: valid GUID pointers and out-pointer.
        unsafe { f(&CLSID, &IClassFactory::IID, &mut ppv) }.ok().expect("DllGetClassObject");
        // SAFETY: ppv holds an IClassFactory reference we now own.
        unsafe { IClassFactory::from_raw(ppv) }
    }

    pub fn can_unload(&self) -> HRESULT {
        let f: NoArgs = self.sym("DllCanUnloadNow");
        // SAFETY: no arguments.
        unsafe { f() }
    }
}

pub fn data_object(paths: &[&Path]) -> IDataObject {
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

pub struct Menu(windows::Win32::UI::WindowsAndMessaging::HMENU);

impl Drop for Menu {
    fn drop(&mut self) {
        // SAFETY: handle from CreatePopupMenu.
        let _ = unsafe { DestroyMenu(self.0) };
    }
}

/// Same as `query`, but with an explicit id range.
pub fn query_range(ctx: &IContextMenu, first: u32, last: u32) -> Vec<(String, bool, u32)> {
    // SAFETY: plain menu creation.
    let menu = Menu(unsafe { CreatePopupMenu() }.unwrap());
    // SAFETY: menu is live; id range is ours.
    let hr = unsafe { ctx.QueryContextMenu(menu.0, 0, first, last, 0) };
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
            let id = unsafe { GetMenuItemID(menu.0, pos as i32) };
            (String::from_utf16_lossy(&buf[..len]), state & MF_GRAYED.0 == 0, id)
        })
        .collect()
}

/// (label, enabled) per inserted item.
pub fn query(ctx: &IContextMenu) -> Vec<(String, bool)> {
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

pub fn invoke(ctx: &IContextMenu, offset: usize) {
    let info = CMINVOKECOMMANDINFO {
        cbSize: std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32,
        lpVerb: PCSTR(offset as *const u8),
        ..Default::default()
    };
    // SAFETY: info is fully initialized; lpVerb carries an offset (HIWORD == 0).
    unsafe { ctx.InvokeCommand(&info) }.expect("InvokeCommand");
}

pub fn verb(ctx: &IContextMenu, offset: usize) -> String {
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

pub fn context_menu(dll: &Dll, paths: &[&Path]) -> IContextMenu {
    let init: IShellExtInit =
        unsafe { dll.factory().CreateInstance(None) }.expect("CreateInstance");
    let data = data_object(paths);
    // SAFETY: data is a live data object; pidl/hkey may be null.
    unsafe { init.Initialize(None, &data, None) }.expect("Initialize");
    init.cast().expect("IContextMenu")
}

pub struct Guard(pub PathBuf);

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = unlock_tree(&self.0);
    }
}
