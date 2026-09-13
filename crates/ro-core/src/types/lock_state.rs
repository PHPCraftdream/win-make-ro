#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    /// No lock ACE at all.
    Unlocked,
    /// Lock ACE set directly on this item.
    Explicit,
    /// Lock ACE only inherited from an ancestor.
    Inherited,
}
