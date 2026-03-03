use std::collections::HashSet;
use std::path::PathBuf;

fn default_http_port() -> u16 {
    80
}

fn default_https_port() -> u16 {
    443
}

fn default_dns_port() -> u16 {
    1053
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    #[serde(default = "default_https_port")]
    pub https_port: u16,

    #[serde(default = "default_dns_port")]
    pub dns_port: u16,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            http_port: default_http_port(),
            https_port: default_https_port(),
            dns_port: default_dns_port(),
            log_level: default_log_level(),
        }
    }
}

impl DaemonConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.http_port == 0 {
            return Err("http_port cannot be 0".into());
        }
        if self.https_port == 0 {
            return Err("https_port cannot be 0".into());
        }
        if self.dns_port == 0 {
            return Err("dns_port cannot be 0".into());
        }
        if self.http_port == self.https_port {
            return Err("http_port and https_port must be different".into());
        }

        let ports = [self.http_port, self.https_port, self.dns_port];
        let unique_ports: HashSet<_> = ports.iter().collect();
        if unique_ports.len() != ports.len() {
            return Err("http_port, https_port, and dns_port must all be different".into());
        }

        let valid_levels = ["error", "warn", "info", "debug"];
        if !valid_levels.contains(&self.log_level.as_str()) {
            return Err(format!(
                "Invalid log_level '{}'. Must be one of: {}",
                self.log_level,
                valid_levels.join(", ")
            ));
        }

        Ok(())
    }
}

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
