mod class_factory;
mod hdrop_paths;
mod menu_ext;
mod module;

pub use class_factory::ClassFactory;
pub use hdrop_paths::hdrop_paths;
pub use menu_ext::MenuExt;
pub use module::Module;

use windows::core::GUID;

/// Must match `ro_register::CLSID` (checked by a test).
pub const CLSID_MENU_EXT: GUID = GUID::from_u128(0x7A3C1F0E_5B2D_4E8A_9C61_0D4F2B7E9A11);

#[cfg(test)]
mod tests {
    use super::CLSID_MENU_EXT;

    #[test]
    fn clsid_matches_registry_string() {
        assert_eq!(format!("{{{CLSID_MENU_EXT:?}}}"), ro_register::CLSID);
    }
}
