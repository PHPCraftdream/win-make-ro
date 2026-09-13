use windows::Win32::Foundation::S_OK;
use windows::Win32::System::Ole::SELFREG_E_CLASS;
use windows::core::HRESULT;

/// `regsvr32 /u ro_shellext.dll`
#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    match ro_register::uninstall() {
        Ok(()) => S_OK,
        Err(_) => SELFREG_E_CLASS,
    }
}
