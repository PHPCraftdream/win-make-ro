use std::path::PathBuf;

use ro_core::{LockState, lock_state};

use crate::cli::{EXIT_ERRORS, EXIT_OK};

/// Prints `<state>\t<path>` per line. Exit 1 if any path could not be read.
pub fn status(paths: &[PathBuf]) -> i32 {
    let mut code = EXIT_OK;
    for p in paths {
        match lock_state(p) {
            Ok(s) => println!("{}\t{}", label(s), p.display()),
            Err(e) => {
                println!("error\t{}", p.display());
                eprintln!("{e}");
                code = EXIT_ERRORS;
            }
        }
    }
    code
}

fn label(s: LockState) -> &'static str {
    match s {
        LockState::Unlocked => "unlocked",
        LockState::Explicit => "locked",
        LockState::Inherited => "inherited",
    }
}
