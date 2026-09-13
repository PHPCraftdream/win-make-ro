use std::sync::OnceLock;

use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, HBITMAP, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DI_NORMAL, DestroyIcon, DrawIconEx, GetSystemMetrics, HICON, IMAGE_ICON, LR_DEFAULTCOLOR,
    LoadImageW, SM_CXSMICON, SM_CYSMICON,
};
use windows::core::PCWSTR;

use crate::com::Module;

/// Resource id of the icon in `assets/app.rc`.
const ICON_ID: u16 = 1;

static BITMAP: OnceLock<isize> = OnceLock::new();

/// 32-bit ARGB bitmap of the app icon at small-icon size, for `hbmpItem`.
/// Created once per process and kept alive for the DLL's lifetime.
pub fn menu_bitmap() -> Option<HBITMAP> {
    let raw = *BITMAP.get_or_init(|| build().map_or(0, |b| b.0 as isize));
    (raw != 0).then_some(HBITMAP(raw as *mut _))
}

fn build() -> Option<HBITMAP> {
    let hinst = HINSTANCE(Module::handle().0);
    // SAFETY: plain metrics query.
    let (cx, cy) = unsafe { (GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON)) };
    // SAFETY: MAKEINTRESOURCE(ICON_ID); hinst is our module.
    let icon = unsafe {
        LoadImageW(
            Some(hinst),
            PCWSTR(ICON_ID as usize as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR,
        )
    }
    .ok()?;
    let icon = HICON(icon.0);

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: cx,
            biHeight: -cy, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    // SAFETY: info is a valid 32bpp BITMAPINFO; bits receives the pixel buffer.
    unsafe {
        let dc = CreateCompatibleDC(None);
        let bmp = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0);
        let bmp = match bmp {
            Ok(b) => b,
            Err(_) => {
                let _ = DeleteDC(dc);
                DestroyIcon(icon).ok();
                return None;
            }
        };
        let old = SelectObject(dc, bmp.into());
        // Drawing onto a 32bpp DIB keeps the icon's alpha channel.
        let drawn = DrawIconEx(dc, 0, 0, icon, cx, cy, 0, None, DI_NORMAL).is_ok();
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        DestroyIcon(icon).ok();
        if drawn {
            Some(bmp)
        } else {
            let _ = DeleteObject(bmp.into());
            None
        }
    }
}
