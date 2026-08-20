//! Installation of the OS-managed, unprivileged Roxy daemon.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use std::env;
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::config::DaemonConfig;
use crate::infrastructure::paths::RoxyPaths;

#[derive(Debug, Clone)]
pub struct RuntimeUser {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}

impl RuntimeUser {
    fn detect() -> Result<Self> {
        ensure_root()?;

        let name = env::var("SUDO_USER")
            .ok()
            .filter(|name| name != "root")
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Cannot determine the developer account. Run 'sudo roxy install' \
                     from the account that should own Roxy."
                )
            })?;
        let uid = env::var("SUDO_UID")
            .context("sudo did not provide SUDO_UID")?
            .parse()
            .context("SUDO_UID is invalid")?;
        let gid = env::var("SUDO_GID")
            .context("sudo did not provide SUDO_GID")?
            .parse()
            .context("SUDO_GID is invalid")?;

        Ok(Self {
            name,
            uid,
            gid,
            home: crate::config::runtime_home_dir(),
        })
    }
}

pub fn install(
    config_path: &Path,
    paths: &RoxyPaths,
    daemon: &DaemonConfig,
) -> Result<RuntimeUser> {
    let user = RuntimeUser::detect()?;
    transfer_runtime_ownership(config_path, paths, &user)?;

    let executable = service_executable()?;
    platform_install(&executable, config_path, paths, daemon, &user)?;
    Ok(user)
}

/// Prefer the path used to invoke Roxy when it resolves to this process. This
/// preserves stable package-manager symlinks across upgrades instead of
/// embedding a versioned Cellar or installation path in the service unit.
fn service_executable() -> Result<PathBuf> {
    let current = env::current_exe().context("Failed to locate the Roxy executable")?;
    let canonical_current = current.canonicalize().unwrap_or_else(|_| current.clone());
    let Some(invoked) = env::args_os().next().map(PathBuf::from) else {
        return Ok(current);
    };

    let mut candidates: Vec<PathBuf> = if invoked.components().count() > 1 {
        if invoked.is_absolute() {
            vec![invoked]
        } else {
            vec![
                env::current_dir()
                    .context("Failed to resolve the invoked Roxy path")?
                    .join(invoked),
            ]
        }
    } else {
        env::var_os("PATH")
            .map(|path| {
                env::split_paths(&path)
                    .map(|dir| dir.join(&invoked))
                    .collect()
            })
            .unwrap_or_default()
    };
    candidates.extend(
        [
            "/opt/homebrew/bin/roxy",
            "/usr/local/bin/roxy",
            "/home/linuxbrew/.linuxbrew/bin/roxy",
        ]
        .into_iter()
        .map(PathBuf::from),
    );

    Ok(candidates
        .into_iter()
        .find(|candidate| {
            candidate
                .canonicalize()
                .is_ok_and(|resolved| resolved == canonical_current)
        })
        .unwrap_or(current))
}

/// Validate the privileged installer context and user-owned paths before
/// changing system state.
pub fn validate_install_invocation(config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let user = RuntimeUser::detect()?;
    validate_runtime_paths(config_path, paths, &user)
}

pub fn uninstall() -> Result<()> {
    ensure_root()?;
    platform_uninstall()
}

pub fn is_installed() -> bool {
    platform_is_installed()
}

/// Trigger an installed socket-activated service without elevated privileges.
pub fn activate(http_port: u16) -> Result<()> {
    TcpStream::connect_timeout(
        &SocketAddrV4::new(Ipv4Addr::LOCALHOST, http_port).into(),
        std::time::Duration::from_secs(2),
    )
    .context("Failed to activate the Roxy service through its HTTP socket")?;
    Ok(())
}

fn ensure_root() -> Result<()> {
    let output = Command::new("id")
        .arg("-u")
        .output()
        .context("Failed to determine current user ID")?;
    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "0" {
        bail!("System installation requires root privileges. Run: sudo roxy install");
    }
    Ok(())
}

fn transfer_runtime_ownership(
    config_path: &Path,
    paths: &RoxyPaths,
    user: &RuntimeUser,
) -> Result<()> {
    let mut directories: Vec<PathBuf> = vec![paths.data_dir.clone()];
    if let Some(path) = config_path.parent() {
        directories.push(path.to_path_buf());
    }
    if let Some(path) = paths.pid_file.parent() {
        directories.push(path.to_path_buf());
    }
    if let Some(path) = paths.log_file.parent() {
        directories.push(path.to_path_buf());
    }
    if let Some(path) = paths.socket_path.parent() {
        directories.push(path.to_path_buf());
    }
    directories.sort();
    directories.dedup();

    let files = [
        config_path.to_path_buf(),
        paths.data_dir.join("ca.crt"),
        paths.data_dir.join("ca.key"),
        paths.pid_file.clone(),
        paths.log_file.clone(),
        paths.socket_path.clone(),
    ];
    validate_targets(directories.iter().chain(files.iter()), &user.home)?;

    for target in directories
        .iter()
        .chain(files.iter())
        .filter(|path| path.exists())
    {
        chown(target, user)?;
    }

    Ok(())
}

fn validate_runtime_paths(config_path: &Path, paths: &RoxyPaths, user: &RuntimeUser) -> Result<()> {
    let targets = [
        config_path.to_path_buf(),
        paths.data_dir.clone(),
        paths.pid_file.clone(),
        paths.log_file.clone(),
        paths.socket_path.clone(),
    ];
    validate_targets(targets.iter(), &user.home)
}

fn validate_targets<'a>(targets: impl Iterator<Item = &'a PathBuf>, home: &Path) -> Result<()> {
    let canonical_home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    for target in targets {
        let mut existing = target.as_path();
        while !existing.exists() {
            existing = existing.parent().ok_or_else(|| {
                anyhow::anyhow!("Cannot resolve Roxy state path {}", target.display())
            })?;
        }
        let resolves_below_home = existing
            .canonicalize()
            .is_ok_and(|resolved| resolved.starts_with(&canonical_home));
        if !target.starts_with(home) || target == home || !resolves_below_home {
            bail!(
                "Unprivileged Roxy state must be stored below {} (got {}). \
                 Choose a user-owned --config and [paths] location.",
                home.display(),
                target.display()
            );
        }
    }
    Ok(())
}

fn chown(target: &Path, user: &RuntimeUser) -> Result<()> {
    let owner = format!("{}:{}", user.uid, user.gid);
    let output = Command::new("chown")
        .arg(&owner)
        .arg(target)
        .output()
        .with_context(|| format!("Failed to change ownership of {}", target.display()))?;
    if !output.status.success() {
        bail!(
            "Failed to make {} user-owned: {}",
            target.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn platform_install(
    executable: &Path,
    config_path: &Path,
    paths: &RoxyPaths,
    daemon: &DaemonConfig,
    user: &RuntimeUser,
) -> Result<()> {
    macos::install(executable, config_path, paths, daemon, user)
}

#[cfg(target_os = "linux")]
fn platform_install(
    executable: &Path,
    config_path: &Path,
    paths: &RoxyPaths,
    daemon: &DaemonConfig,
    user: &RuntimeUser,
) -> Result<()> {
    linux::install(executable, config_path, paths, daemon, user)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_install(
    _executable: &Path,
    _config_path: &Path,
    _paths: &RoxyPaths,
    _daemon: &DaemonConfig,
    _user: &RuntimeUser,
) -> Result<()> {
    bail!("Automatic service installation is not supported on this operating system")
}

#[cfg(target_os = "macos")]
fn platform_uninstall() -> Result<()> {
    macos::uninstall()
}

#[cfg(target_os = "macos")]
fn platform_is_installed() -> bool {
    macos::is_installed()
}

#[cfg(target_os = "linux")]
fn platform_is_installed() -> bool {
    linux::is_installed()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_is_installed() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn platform_uninstall() -> Result<()> {
    linux::uninstall()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_uninstall() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_paths_must_remain_below_the_user_home() {
        let home = tempfile::tempdir().unwrap();
        let user = RuntimeUser {
            name: "dev".into(),
            uid: 1000,
            gid: 1000,
            home: home.path().to_path_buf(),
        };
        let paths = RoxyPaths {
            data_dir: home.path().join(".local/share/roxy"),
            pid_file: home.path().join(".local/state/roxy/run/roxy.pid"),
            log_file: home.path().join(".local/state/roxy/roxy.log"),
            socket_path: home.path().join(".local/state/roxy/run/roxy.sock"),
        };

        assert!(
            validate_runtime_paths(&home.path().join(".config/roxy/config.toml"), &paths, &user)
                .is_ok()
        );
        assert!(validate_runtime_paths(Path::new("/etc/roxy.toml"), &paths, &user).is_err());
    }
}
