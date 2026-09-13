use std::path::PathBuf;

use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::{CF_HDROP, ReleaseStgMedium};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::core::Result;

/// Extracts the selected paths from a shell data object (CF_HDROP).
pub fn hdrop_paths(data: &IDataObject) -> Result<Vec<PathBuf>> {
    let fmt = FORMATETC {
        cfFormat: CF_HDROP.0,
        ptd: std::ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: -1,
        tymed: TYMED_HGLOBAL.0 as u32,
    };
    // SAFETY: fmt is a fully initialized FORMATETC.
    let mut medium = unsafe { data.GetData(&fmt) }?;
    // SAFETY: medium.tymed == TYMED_HGLOBAL, so `u.hGlobal` is the active field.
    let hglobal = unsafe { medium.u.hGlobal };
    // SAFETY: hglobal is a live HGLOBAL owned by `medium` until released below.
    let ptr = unsafe { GlobalLock(hglobal) };
    let paths = if ptr.is_null() { Vec::new() } else { read_drop(HDROP(ptr)) };
    // SAFETY: matching unlock/release for the calls above.
    unsafe {
        let _ = GlobalUnlock(hglobal);
        ReleaseStgMedium(&mut medium);
    }
    Ok(paths)
}

fn read_drop(hdrop: HDROP) -> Vec<PathBuf> {
    // SAFETY: hdrop points at a locked DROPFILES block for every call below.
    unsafe {
        let count = DragQueryFileW(hdrop, u32::MAX, None);
        (0..count)
            .filter_map(|i| {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                if len == 0 {
                    return None;
                }
                let mut buf = vec![0u16; len + 1];
                let n = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
            })
            .collect()
    }
}
