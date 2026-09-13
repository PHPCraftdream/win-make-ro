use windows::Win32::Storage::FileSystem::{
    DELETE, FILE_APPEND_DATA, FILE_DELETE_CHILD, FILE_WRITE_ATTRIBUTES, FILE_WRITE_DATA,
    FILE_WRITE_EA,
};

/// Rights denied to `Everyone` by the lock. The exact value is the lock's signature.
pub const LOCK_MASK: u32 = FILE_WRITE_DATA.0
    | FILE_APPEND_DATA.0
    | FILE_WRITE_EA.0
    | FILE_WRITE_ATTRIBUTES.0
    | DELETE.0
    | FILE_DELETE_CHILD.0;
