use windows::Win32::Foundation::CLASS_E_NOAGGREGATION;
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl};
use windows::core::{BOOL, GUID, IUnknown, Interface, Ref, Result, implement};

use super::{MenuExt, Module};

#[implement(IClassFactory)]
pub struct ClassFactory;

impl ClassFactory {
    /// The factory itself is a COM object handed to the caller, so it keeps
    /// the DLL loaded exactly like the objects it creates.
    pub fn new() -> Self {
        Module::object_created();
        Self
    }
}

impl Drop for ClassFactory {
    fn drop(&mut self) {
        Module::object_dropped();
    }
}

impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        if punkouter.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let obj: IUnknown = MenuExt::new().into();
        // SAFETY: riid/ppvobject are valid per the COM contract.
        unsafe { obj.query(riid, ppvobject) }.ok()
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        Module::set_server_lock(flock.as_bool());
        Ok(())
    }
}
