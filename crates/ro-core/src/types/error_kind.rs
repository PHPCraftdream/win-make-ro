use std::io;

#[derive(Debug)]
pub enum ErrorKind {
    Os(io::Error),
    /// Item is locked only through inheritance; unlock the ancestor instead.
    LockedByParent,
    /// Symlinks/junctions are never touched (ACL would land on the target).
    ReparsePoint,
    /// The item has a NULL DACL. Locking it would have to replace that with a
    /// real ACL, and no later unlock could prove the original was NULL, so the
    /// item is left alone rather than silently reshaped.
    NullDacl,
}
