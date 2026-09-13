use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};

use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

static HANDLE: AtomicIsize = AtomicIsize::new(0);
static LIVE: AtomicUsize = AtomicUsize::new(0);

/// Process-wide DLL state: module handle and live COM object count.
pub struct Module;

impl Module {
    pub fn set_handle(h: HMODULE) {
        HANDLE.store(h.0 as isize, Ordering::Release);
    }

    pub fn handle() -> HMODULE {
        HMODULE(HANDLE.load(Ordering::Acquire) as *mut _)
    }

    pub fn dll_path() -> Option<PathBuf> {
        let h = Self::handle();
        if h.0.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 32 * 1024];
        // SAFETY: buf is a valid writable slice; h is our own module handle.
        let n = unsafe { GetModuleFileNameW(Some(h), &mut buf) } as usize;
        if n == 0 || n >= buf.len() {
            return None;
        }
        Some(PathBuf::from(String::from_utf16_lossy(&buf[..n])))
    }

    /// Helper executable expected next to the DLL.
    pub fn helper_path() -> Option<PathBuf> {
        Self::dll_path().map(|p| p.with_file_name("win-make-ro.exe"))
    }

    pub fn object_created() {
        LIVE.fetch_add(1, Ordering::AcqRel);
    }

    pub fn object_dropped() {
        LIVE.fetch_sub(1, Ordering::AcqRel);
    }

    pub fn live_objects() -> usize {
        LIVE.load(Ordering::Acquire)
    }
}
