use std::io;
use std::path::Path;

/// Sets or clears `FILE_ATTRIBUTE_READONLY`, reporting whether it changed.
///
/// Windows refuses to delete or rename a file carrying this attribute even
/// when the caller holds `FILE_DELETE_CHILD` on the parent folder, which would
/// otherwise bypass a `DELETE` deny on the file itself. Our deny on
/// `FILE_WRITE_ATTRIBUTES` then keeps the attribute in place.
pub fn set_readonly_attr(path: &Path, on: bool) -> io::Result<bool> {
    let mut perms = std::fs::symlink_metadata(path)?.permissions();
    if perms.readonly() == on {
        return Ok(false);
    }
    perms.set_readonly(on);
    std::fs::set_permissions(path, perms)?;
    Ok(true)
}
