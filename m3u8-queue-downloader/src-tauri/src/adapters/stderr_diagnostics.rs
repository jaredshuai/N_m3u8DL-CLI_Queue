use crate::ports::diagnostics::Diagnostics;

pub(crate) struct StderrDiagnostics;

impl Diagnostics for StderrDiagnostics {
    fn warn(&self, message: &str) {
        eprintln!("{message}");
    }
}
