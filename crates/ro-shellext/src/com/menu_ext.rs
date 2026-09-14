use std::path::PathBuf;
use std::sync::Mutex;

use windows::Win32::Foundation::{E_FAIL, E_INVALIDARG};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::System::Com::IDataObject;
use windows::Win32::System::Registry::HKEY;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    CMF_DEFAULTONLY, CMINVOKECOMMANDINFO, CMINVOKECOMMANDINFOEX, GCS_HELPTEXTW, GCS_VERBW,
    IContextMenu, IContextMenu_Impl, IShellExtInit, IShellExtInit_Impl, SEE_MASK_UNICODE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HMENU, InsertMenuW, MENUITEMINFOW, MF_BYPOSITION, MF_GRAYED, MF_STRING, MIIM_BITMAP,
    SetMenuItemInfoW,
};
use windows::core::{HRESULT, PCWSTR, PSTR, Ref, Result, implement};

use super::{Module, hdrop_paths};
use crate::menu::{Item, MenuIcon, Selection, launch, plan};

/// One instance per context-menu invocation (Apartment threaded).
#[implement(IShellExtInit, IContextMenu)]
pub struct MenuExt {
    paths: Mutex<Vec<PathBuf>>,
    items: Mutex<Vec<Item>>,
    /// Owned for this object's lifetime, which is at least as long as the menu
    /// Explorer built from it. Freed on drop, so nothing accumulates.
    icon: Mutex<Option<MenuIcon>>,
}

impl MenuExt {
    pub fn new() -> Self {
        Module::object_created();
        Self {
            paths: Mutex::new(Vec::new()),
            items: Mutex::new(Vec::new()),
            icon: Mutex::new(None),
        }
    }

    fn lock_paths(&self) -> Vec<PathBuf> {
        self.paths.lock().map(|p| p.clone()).unwrap_or_default()
    }

    /// Creates the icon bitmap once per object and hands out its handle.
    fn bitmap(&self) -> Option<HBITMAP> {
        let mut slot = self.icon.lock().ok()?;
        if slot.is_none() {
            *slot = MenuIcon::new();
        }
        slot.as_ref().map(MenuIcon::bitmap)
    }

    fn lock_items(&self) -> Vec<Item> {
        self.items.lock().map(|i| i.clone()).unwrap_or_default()
    }
}

impl Drop for MenuExt {
    fn drop(&mut self) {
        Module::object_dropped();
    }
}

impl IShellExtInit_Impl for MenuExt_Impl {
    fn Initialize(
        &self,
        _pidl: *const ITEMIDLIST,
        pdtobj: Ref<IDataObject>,
        _hkey: HKEY,
    ) -> Result<()> {
        let data = pdtobj.ok()?;
        let paths = hdrop_paths(data)?;
        if paths.is_empty() {
            return Err(E_INVALIDARG.into());
        }
        *self.paths.lock().map_err(|_| E_FAIL)? = paths;
        Ok(())
    }
}

impl IContextMenu_Impl for MenuExt_Impl {
    fn QueryContextMenu(
        &self,
        hmenu: HMENU,
        indexmenu: u32,
        idcmdfirst: u32,
        idcmdlast: u32,
        uflags: u32,
    ) -> HRESULT {
        if uflags & CMF_DEFAULTONLY != 0 {
            return HRESULT(0);
        }
        // idCmdLast is inclusive; using an id past it would collide with
        // another handler's commands. A last below first leaves no room.
        let room = idcmdlast.checked_sub(idcmdfirst).map_or(0, |n| n.saturating_add(1) as usize);
        let mut items = plan(&Selection::inspect(&self.lock_paths()));
        items.truncate(room);
        for (i, item) in items.iter().enumerate() {
            let text: Vec<u16> = item.text().encode_utf16().chain(Some(0)).collect();
            let flags = if item.enabled() { MF_STRING } else { MF_STRING | MF_GRAYED };
            // SAFETY: text is NUL-terminated and outlives the call.
            let r = unsafe {
                InsertMenuW(
                    hmenu,
                    indexmenu + i as u32,
                    MF_BYPOSITION | flags,
                    (idcmdfirst + i as u32) as usize,
                    PCWSTR(text.as_ptr()),
                )
            };
            if r.is_err() {
                return E_FAIL;
            }
            if let Some(bmp) = self.bitmap() {
                let mii = MENUITEMINFOW {
                    cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                    fMask: MIIM_BITMAP,
                    hbmpItem: bmp,
                    ..Default::default()
                };
                // SAFETY: mii is fully initialized; item was just inserted at this position.
                let _ = unsafe { SetMenuItemInfoW(hmenu, indexmenu + i as u32, true, &mii) };
            }
        }
        let count = items.len() as i32;
        if let Ok(mut slot) = self.items.lock() {
            *slot = items;
        }
        HRESULT(count)
    }

    fn InvokeCommand(&self, pici: *const CMINVOKECOMMANDINFO) -> Result<()> {
        // SAFETY: COM passes a valid, at least CMINVOKECOMMANDINFO-sized struct.
        let info = unsafe { &*pici };
        let items = self.lock_items();
        // With CMIC_MASK_UNICODE (spelled SEE_MASK_UNICODE here) the caller
        // passes the larger CMINVOKECOMMANDINFOEX, which carries a second,
        // wide verb field. A *string* verb may then arrive in lpVerbW, but the
        // numeric id always stays in lpVerb: lpVerbW is NULL for it, so
        // reading the id from there would run the first item instead.
        let wide_verb = (info.fMask & SEE_MASK_UNICODE != 0
            && info.cbSize as usize >= std::mem::size_of::<CMINVOKECOMMANDINFOEX>())
        .then(|| {
            // SAFETY: the mask and cbSize together promise the Ex layout.
            unsafe { &*(pici as *const CMINVOKECOMMANDINFOEX) }
        })
        .filter(|ex| ex.lpVerbW.0 as usize >> 16 != 0);
        let ansi_verb = info.lpVerb.0 as usize;
        let item = if let Some(ex) = wide_verb {
            // SAFETY: lpVerbW is a NUL-terminated wide string in that case.
            let s = unsafe { ex.lpVerbW.to_string() }.unwrap_or_default();
            items.iter().copied().find(|i| i.verb() == s)
        } else if ansi_verb >> 16 != 0 {
            // SAFETY: lpVerb is a NUL-terminated ANSI string in that case.
            let s = unsafe { info.lpVerb.to_string() }.unwrap_or_default();
            items.iter().copied().find(|i| i.verb() == s)
        } else {
            items.get(ansi_verb & 0xFFFF).copied()
        };
        let item = item.filter(|i| i.enabled()).ok_or(E_INVALIDARG)?;
        let helper = Module::helper_path().ok_or(E_FAIL)?;
        launch(&helper, item, &self.lock_paths()).map_err(|_| E_FAIL.into())
    }

    fn GetCommandString(
        &self,
        idcmd: usize,
        utype: u32,
        _reserved: *const u32,
        pszname: PSTR,
        cchmax: u32,
    ) -> Result<()> {
        // cchMax counts characters of the caller's buffer, terminator
        // included, so nothing at all may be written when it is zero.
        if cchmax == 0 {
            return Err(E_INVALIDARG.into());
        }
        let item = self.lock_items().get(idcmd).copied().ok_or(E_INVALIDARG)?;
        let text = match utype {
            GCS_VERBW => item.verb(),
            GCS_HELPTEXTW => item.help(),
            _ => return Err(E_INVALIDARG.into()),
        };
        let wide: Vec<u16> = text.encode_utf16().take(cchmax as usize - 1).chain(Some(0)).collect();
        // SAFETY: for the *W types pszname is a u16 buffer of cchmax chars.
        unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), pszname.0 as *mut u16, wide.len()) };
        Ok(())
    }
}
