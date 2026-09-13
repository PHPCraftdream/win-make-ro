//! Explorer context-menu handler (IShellExtInit + IContextMenu).
//! Decides which items to show from the selection's lock state and delegates
//! the work to `win-make-ro.exe` next to this DLL.
//!
//! ASSUMES: windows 0.62.2 (Cargo.lock); classic Win32 context menu.

#![cfg(windows)]
// LNK4104 about DllCanUnloadNow etc. being non-PRIVATE is benign for a cdylib.
#![allow(linker_messages)]

mod com;
mod exports;
mod menu;

pub use exports::{
    DllCanUnloadNow, DllGetClassObject, DllMain, DllRegisterServer, DllUnregisterServer,
};
