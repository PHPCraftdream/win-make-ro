use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_FLUSH, SHCNF_IDLIST, SHChangeNotify};

/// Tells Explorer to re-read shell associations.
pub fn notify() {
    // SAFETY: no pointers passed; documented as safe with SHCNF_IDLIST + nulls.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST | SHCNF_FLUSH, None, None) };
}
