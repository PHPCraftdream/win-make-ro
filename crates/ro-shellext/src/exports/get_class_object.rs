use windows::Win32::Foundation::CLASS_E_CLASSNOTAVAILABLE;
use windows::Win32::System::Com::IClassFactory;
use windows::core::{GUID, HRESULT, Interface};

use crate::com::{CLSID_MENU_EXT, ClassFactory};

/// # Safety
/// COM entry point: `rclsid`/`riid` point at GUIDs, `ppv` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    // SAFETY: COM guarantees valid pointers for these arguments.
    if unsafe { *rclsid } != CLSID_MENU_EXT {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory: IClassFactory = ClassFactory.into();
    // SAFETY: riid/ppv are valid per the COM contract.
    unsafe { factory.query(riid, ppv) }
}
