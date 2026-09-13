mod error;
mod error_kind;
mod lock_mask;
mod lock_state;
mod report;

pub use error::Error;
pub use error_kind::ErrorKind;
pub use lock_mask::LOCK_MASK;
pub use lock_state::LockState;
pub use report::Report;

pub type Result<T> = std::result::Result<T, Error>;
