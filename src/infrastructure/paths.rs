use std::path::PathBuf;

fn default_data_dir() -> PathBuf {
    PathBuf::from("/etc/roxy")
}

fn default_pid_file() -> PathBuf {
    PathBuf::from("/var/run/roxy.pid")
}

fn default_log_file() -> PathBuf {
    PathBuf::from("/var/log/roxy/roxy.log")
}

fn default_certs_dir() -> PathBuf {
    PathBuf::from("/etc/roxy/certs")
}

/// All resolved paths needed by Roxy components.
/// Loaded once from config, then passed to components via DI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoxyPaths {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    #[serde(default = "default_pid_file")]
    pub pid_file: PathBuf,

    #[serde(default = "default_log_file")]
    pub log_file: PathBuf,

    #[serde(default = "default_certs_dir")]
    pub certs_dir: PathBuf,
}

impl Default for RoxyPaths {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            pid_file: default_pid_file(),
            log_file: default_log_file(),
            certs_dir: default_certs_dir(),
        }
    }
}
