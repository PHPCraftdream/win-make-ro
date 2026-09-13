//! Writes to a scratch key under HKCU so the real registration is untouched.

use std::path::Path;

use ro_register::{CLSID, HANDLER_NAME, install_under, is_installed_under, uninstall_under};
use windows_registry::{CURRENT_USER, Key};

struct Scratch {
    path: String,
    key: Key,
}

impl Scratch {
    fn new() -> Self {
        let path = format!(r"Software\WinMakeRO-test\{}", std::process::id());
        let key = CURRENT_USER.create(&path).unwrap();
        Self { path, key }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = CURRENT_USER.remove_tree(&self.path);
    }
}

#[test]
fn install_writes_all_keys_and_uninstall_removes_them() {
    let s = Scratch::new();
    let dll = Path::new(r"C:\some where\ro_shellext.dll");
    assert!(is_installed_under(&s.key).is_none());

    install_under(&s.key, dll).unwrap();
    assert_eq!(is_installed_under(&s.key).as_deref(), Some(dll));
    let inproc = s.key.open(format!(r"CLSID\{CLSID}\InprocServer32")).unwrap();
    assert_eq!(inproc.get_string("ThreadingModel").unwrap(), "Apartment");
    for t in ["*", "Directory"] {
        let k = s.key.open(format!(r"{t}\shellex\ContextMenuHandlers\{HANDLER_NAME}")).unwrap();
        assert_eq!(k.get_string("").unwrap(), CLSID);
    }

    // Re-install overwrites the path.
    let dll2 = Path::new(r"D:\ro_shellext.dll");
    install_under(&s.key, dll2).unwrap();
    assert_eq!(is_installed_under(&s.key).as_deref(), Some(dll2));

    uninstall_under(&s.key).unwrap();
    assert!(is_installed_under(&s.key).is_none());
    assert!(s.key.open(format!(r"*\shellex\ContextMenuHandlers\{HANDLER_NAME}")).is_err());
    // Second uninstall is a no-op, not an error.
    uninstall_under(&s.key).unwrap();
}
