use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::process::Command;

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

/// Home directory of the account that owns the Roxy runtime.
///
/// `sudo` normally changes `HOME`, so installation resolves the original
/// account from `SUDO_USER`. Everyday commands simply use `HOME`.
pub(crate) fn runtime_home_dir() -> PathBuf {
    if let Some(user) = env::var_os("SUDO_USER").filter(|value| value != "root")
        && let Some(home) = lookup_home_dir(&user.to_string_lossy())
    {
        return home;
    }

    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(target_os = "macos")]
fn lookup_home_dir(user: &str) -> Option<PathBuf> {
    let output = Command::new("dscl")
        .args([".", "-read", &format!("/Users/{user}"), "NFSHomeDirectory"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    value.split_whitespace().last().map(PathBuf::from)
}

#[cfg(target_os = "linux")]
fn lookup_home_dir(user: &str) -> Option<PathBuf> {
    let output = Command::new("getent")
        .args(["passwd", user])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    value.trim().split(':').nth(5).map(PathBuf::from)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn lookup_home_dir(_user: &str) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
fn default_data_dir() -> PathBuf {
    runtime_home_dir().join("Library/Application Support/Roxy")
}

#[cfg(target_os = "linux")]
fn default_data_dir() -> PathBuf {
    runtime_home_dir().join(".local/share/roxy")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn default_data_dir() -> PathBuf {
    runtime_home_dir().join(".roxy")
}

#[cfg(target_os = "macos")]
fn default_runtime_dir() -> PathBuf {
    runtime_home_dir().join("Library/Caches/Roxy")
}

#[cfg(target_os = "linux")]
fn default_runtime_dir() -> PathBuf {
    runtime_home_dir().join(".local/state/roxy/run")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn default_runtime_dir() -> PathBuf {
    default_data_dir().join("run")
}

fn default_pid_file() -> PathBuf {
    default_runtime_dir().join("roxy.pid")
}

#[cfg(target_os = "macos")]
fn default_log_file() -> PathBuf {
    runtime_home_dir().join("Library/Logs/Roxy/roxy.log")
}

#[cfg(target_os = "linux")]
fn default_log_file() -> PathBuf {
    runtime_home_dir().join(".local/state/roxy/roxy.log")
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn default_log_file() -> PathBuf {
    default_data_dir().join("roxy.log")
}

fn default_socket_path() -> PathBuf {
    default_runtime_dir().join("roxy.sock")
}

/// Default user-owned configuration file.
#[cfg(target_os = "macos")]
pub fn default_config_path() -> PathBuf {
    default_data_dir().join("config.toml")
}

/// Default user-owned configuration file.
#[cfg(target_os = "linux")]
pub fn default_config_path() -> PathBuf {
    runtime_home_dir().join(".config/roxy/config.toml")
}

/// Default user-owned configuration file.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn default_config_path() -> PathBuf {
    default_data_dir().join("config.toml")
}

/// Docker integration configuration.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DockerConfig {
    /// Enable Docker auto-discovery of containers.
    #[serde(default)]
    pub enabled: bool,
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

    #[serde(default = "default_socket_path")]
    pub socket_path: PathBuf,
}

impl Default for RoxyPaths {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            pid_file: default_pid_file(),
            log_file: default_log_file(),
            socket_path: default_socket_path(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths_are_user_owned() {
        let home = runtime_home_dir();
        let paths = RoxyPaths::default();

        assert!(default_config_path().starts_with(&home));
        assert!(paths.data_dir.starts_with(&home));
        assert!(paths.pid_file.starts_with(&home));
        assert!(paths.log_file.starts_with(&home));
        assert!(paths.socket_path.starts_with(&home));
        assert_ne!(paths.data_dir, PathBuf::from("/etc/roxy"));
    }
}
