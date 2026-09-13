use std::io;

#[derive(Debug)]
pub enum ErrorKind {
    Os(io::Error),
    /// Item is locked only through inheritance; unlock the ancestor instead.
    LockedByParent,
    /// Symlinks/junctions are never touched (ACL would land on the target).
    ReparsePoint,
}
