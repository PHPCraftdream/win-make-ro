//! Per-user (HKCU\Software\Classes) registration of the context-menu handler.
//! Shared by the DLL's `DllRegisterServer` and the helper's `install` command.

#![cfg(windows)]

mod clsid;
mod install;
mod is_installed;
mod notify;
mod uninstall;

pub use clsid::{CLSID, HANDLER_NAME};
pub use install::{install, install_under};
pub use is_installed::{is_installed, is_installed_under};
pub use uninstall::{uninstall, uninstall_under};
