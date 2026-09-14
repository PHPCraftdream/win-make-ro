use super::{Args, Command, EXIT_ERRORS, EXIT_OK};
use crate::ops::{apply, elevate, report_errors, status};
use crate::reinstall::Restart;

pub fn run(args: Args) -> i32 {
    let code = dispatch(&args);
    // Only a list that was handed over is ours to remove, and then whether the
    // work succeeded, failed or went to an elevated child that removed it
    // already. A file the caller named with `--paths-from` is theirs: reading
    // it is no reason to destroy it.
    if let Some(file) = args.paths_file.as_ref().filter(|_| args.consume_paths_file) {
        ro_core::remove_paths_file(file);
    }
    code
}

fn dispatch(args: &Args) -> i32 {
    match args.command {
        Command::Status => status(&args.paths),
        Command::Install => registry(args.gui, install_here()),
        Command::Reinstall => reinstall(args),
        Command::RestartExplorer => registry(args.gui, crate::reinstall::close_and_start()),
        Command::Uninstall => {
            registry(args.gui, ro_register::uninstall().map_err(|e| e.to_string()))
        }
        Command::Lock | Command::Unlock => {
            let report = apply(args.command, &args.paths);
            if report.needs_elevation() && !args.no_elevate {
                return elevate(args);
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

/// Says where the binaries are registered from and what became of Explorer,
/// separately: a restart that did not happen leaves the installation in place,
/// and reporting one failure as the other would be a lie in either direction.
fn reinstall(args: &Args) -> i32 {
    let summary = match crate::reinstall::reinstall(args.to.as_deref()) {
        Ok(s) => s,
        Err(msg) => {
            crate::ops::show_error(args.gui, &msg);
            return EXIT_ERRORS;
        }
    };
    println!("registered {} ({})", summary.to.display(), summary.restart.describe());
    for path in &summary.left_behind {
        eprintln!("{}: superseded copy still in use, left for next time", path.display());
    }
    match &summary.restart {
        Restart::Failed(why) => {
            crate::ops::show_error(
                args.gui,
                // What state Explorer is in depends on where it went wrong,
                // and `why` is the only part that knows: the old shell may be
                // running, or already closed with nothing started in its place.
                &format!(
                    "the installation in {} is registered, but Explorer was not restarted: {why}",
                    summary.to.display()
                ),
            );
            EXIT_ERRORS
        }
        _ => EXIT_OK,
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
