use std::path::PathBuf;

/// All resolved paths needed by Roxy components.
/// Loaded once from config, then passed to components via DI.
#[derive(Debug, Clone)]
pub struct RoxyPaths {
    pub data_dir: PathBuf,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
    pub certs_dir: PathBuf,
}
