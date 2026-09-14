//! A file that carries a list of paths between two of our own processes.
//!
//! A Windows command line stops at 32 767 characters, which a large Explorer
//! selection passes easily, and the process that receives it would never see
//! the tail. The list goes through a file instead: UTF-16 code units with a
//! single NUL between entries, which also keeps names that are not valid
//! Unicode intact — the very reason the command line is built in UTF-16.

mod budget;
mod read;
mod write;

pub use budget::{COMMAND_LINE_BUDGET, fits_command_line};
pub use read::read_paths_file;
pub use write::{remove_paths_file, write_paths_file};
