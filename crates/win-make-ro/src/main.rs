//! Helper executable behind the context menu. Also a plain CLI.
//!
//! ASSUMES: windows 0.62.2, windows-registry 0.6.1 (Cargo.lock).

#![cfg(windows)]

mod cli;
mod ops;

fn main() {
    let code = match cli::Args::parse(std::env::args().skip(1)) {
        Ok(args) => cli::run(args),
        Err(msg) => {
            eprintln!("{msg}\n\n{}", cli::USAGE);
            cli::EXIT_USAGE
        }
    };
    std::process::exit(code);
}
