use windows::Win32::Foundation::{S_FALSE, S_OK};
use windows::core::HRESULT;

use crate::com::Module;

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    if Module::can_unload() { S_OK } else { S_FALSE }
}
