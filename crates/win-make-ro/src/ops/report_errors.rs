use ro_core::Error;

use super::show_error;

const MAX_LINES: usize = 20;

pub fn report_errors(gui: bool, errors: &[Error]) {
    let mut lines: Vec<String> = errors.iter().take(MAX_LINES).map(ToString::to_string).collect();
    if errors.len() > MAX_LINES {
        lines.push(format!("... and {} more", errors.len() - MAX_LINES));
    }
    show_error(gui, &lines.join("\n"));
}
