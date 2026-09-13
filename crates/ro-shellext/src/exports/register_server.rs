use windows::Win32::Foundation::S_OK;
use windows::Win32::System::Ole::SELFREG_E_CLASS;
use windows::core::HRESULT;

use crate::com::Module;

/// `regsvr32 ro_shellext.dll` — per-user registration, no admin needed.
#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    match Module::dll_path().and_then(|p| ro_register::install(&p).ok()) {
        Some(()) => S_OK,
        None => SELFREG_E_CLASS,
    }
}
