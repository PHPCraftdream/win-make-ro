use super::{Error, Result};

/// Outcome of a recursive operation. Errors never abort the walk.
#[derive(Debug, Default)]
pub struct Report {
    pub changed: usize,
    pub skipped: usize,
    pub errors: Vec<Error>,
}

impl Report {
    pub fn needs_elevation(&self) -> bool {
        self.errors.iter().any(Error::is_access_denied)
    }

    /// Folds a single-item result into the report.
    pub fn record(&mut self, r: Result<bool>) {
        match r {
            Ok(true) => self.changed += 1,
            Ok(false) => {}
            Err(e) => self.errors.push(e),
        }
    }
}
