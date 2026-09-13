use super::{Args, Command, EXIT_ERRORS, EXIT_OK};
use crate::ops::{apply, elevate, report_errors, status};

pub fn run(args: Args) -> i32 {
    match args.command {
        Command::Status => status(&args.paths),
        Command::Install => registry(args.gui, install_here()),
        Command::Uninstall => {
            registry(args.gui, ro_register::uninstall().map_err(|e| e.to_string()))
        }
        Command::Lock | Command::Unlock => {
            let report = apply(args.command, &args.paths);
            if report.needs_elevation() && !args.no_elevate {
                return elevate(&args);
            }
            if report.errors.is_empty() {
                EXIT_OK
            } else {
                report_errors(args.gui, &report.errors);
                EXIT_ERRORS
            }
        }
    }
}

fn install_here() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dll = exe.with_file_name("ro_shellext.dll");
    if !dll.is_file() {
        return Err(format!("{} not found next to the executable", dll.display()));
    }
    ro_register::install(&dll).map_err(|e| e.to_string())
}

fn registry(gui: bool, r: Result<(), String>) -> i32 {
    match r {
        Ok(()) => EXIT_OK,
        Err(msg) => {
            crate::ops::show_error(gui, &msg);
            EXIT_ERRORS
        }
    }
}
