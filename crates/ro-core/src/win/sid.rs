use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::Foundation::HLOCAL;
use windows::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows::Win32::Security::{GetLengthSid, PSID};
use windows::core::PCWSTR;

use super::LocalBuf;

const EVERYONE_SID: &str = "S-1-1-0";

pub struct Sid(LocalBuf);

impl Sid {
    pub fn everyone() -> Sid {
        let wide: Vec<u16> = OsStr::new(EVERYONE_SID).encode_wide().chain(Some(0)).collect();
        let mut psid = PSID::default();
        // SAFETY: wide is NUL-terminated; psid receives a LocalAlloc-allocated SID.
        unsafe { ConvertStringSidToSidW(PCWSTR(wide.as_ptr()), &mut psid) }
            .expect("invariant: well-known SID string is valid");
        Sid(LocalBuf(HLOCAL(psid.0)))
    }

    pub fn psid(&self) -> PSID {
        PSID(self.0.0.0)
    }

    pub fn len(&self) -> u32 {
        // SAFETY: valid SID from ConvertStringSidToSidW.
        unsafe { GetLengthSid(self.psid()) }
    }
}
