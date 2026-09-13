use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::DisableThreadLibraryCalls;
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::core::BOOL;

use crate::com::Module;

/// Records the module handle so the DLL can find its own path later.
#[unsafe(no_mangle)]
pub extern "system" fn DllMain(
    module: HMODULE,
    reason: u32,
    _reserved: *mut core::ffi::c_void,
) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        Module::set_handle(module);
        // SAFETY: called on DLL_PROCESS_ATTACH with our own module handle.
        let _ = unsafe { DisableThreadLibraryCalls(module) };
    }
    BOOL(1)
}
