use std::path::Path;

use ro_core::{LockState, lock_state};

/// Above this many items the ACL probe is skipped to keep the menu snappy.
pub const PROBE_LIMIT: usize = 64;

/// Aggregated lock state of the selected items.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    pub any_unlocked: bool,
    pub any_explicit: bool,
    pub any_inherited: bool,
}

impl Selection {
    pub fn inspect<P: AsRef<Path>>(paths: &[P]) -> Self {
        if paths.len() > PROBE_LIMIT {
            return Self { any_unlocked: true, any_explicit: true, any_inherited: false };
        }
        let mut s = Self::default();
        for p in paths {
            // Unreadable items behave as unlocked so the helper can report the error.
            match lock_state(p.as_ref()).unwrap_or(LockState::Unlocked) {
                LockState::Unlocked => s.any_unlocked = true,
                LockState::Explicit => s.any_explicit = true,
                LockState::Inherited => s.any_inherited = true,
            }
        }
        s
    }
}
