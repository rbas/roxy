pub mod install;
pub mod manage_routes;
pub mod register_domain;
pub mod uninstall;
pub mod unregister_domain;

use std::fmt;

/// Outcome of a single step in a multi-step operation.
///
/// Used by application services to report partial success/failure
/// so the CLI layer can render feedback appropriately.
#[derive(Debug, Clone)]
pub enum StepOutcome {
    Success(String),
    Warning(String),
    Skipped(String),
}

impl fmt::Display for StepOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success(msg) => write!(f, "{}", msg),
            Self::Warning(msg) => write!(f, "Warning: {}", msg),
            Self::Skipped(msg) => write!(f, "Skipped: {}", msg),
        }
    }
}
