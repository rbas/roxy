use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::RuntimeUser;
use crate::config::DaemonConfig;
use crate::infrastructure::paths::RoxyPaths;

const SERVICE_PATH: &str = "/etc/systemd/system/roxy.service";
const HTTP_SOCKET_PATH: &str = "/etc/systemd/system/roxy-http.socket";
const HTTPS_SOCKET_PATH: &str = "/etc/systemd/system/roxy-https.socket";

pub fn install(
    executable: &Path,
    config_path: &Path,
    _paths: &RoxyPaths,
    daemon: &DaemonConfig,
    user: &RuntimeUser,
) -> Result<()> {
    fs::write(
        HTTP_SOCKET_PATH,
        render_socket("HTTP", daemon.http_port, "http"),
    )?;
    fs::write(
        HTTPS_SOCKET_PATH,
        render_socket("HTTPS", daemon.https_port, "https"),
    )?;
    fs::write(SERVICE_PATH, render_service(executable, config_path, user))?;

    run_systemctl(["daemon-reload"])?;
    run_systemctl(["stop", "roxy.service"])?;
    run_systemctl([
        "enable",
        "roxy-http.socket",
        "roxy-https.socket",
        "roxy.service",
    ])?;
    run_systemctl(["restart", "roxy-http.socket", "roxy-https.socket"])?;
    run_systemctl(["start", "roxy.service"])?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let _ = Command::new("systemctl")
        .args([
            "disable",
            "--now",
            "roxy.service",
            "roxy-http.socket",
            "roxy-https.socket",
        ])
        .output();
    for path in [SERVICE_PATH, HTTP_SOCKET_PATH, HTTPS_SOCKET_PATH] {
        if Path::new(path).exists() {
            fs::remove_file(path).with_context(|| format!("Failed to remove {path}"))?;
        }
    }
    run_systemctl(["daemon-reload"])?;
    Ok(())
}

pub fn is_installed() -> bool {
    Path::new(SERVICE_PATH).exists()
        && Path::new(HTTP_SOCKET_PATH).exists()
        && Path::new(HTTPS_SOCKET_PATH).exists()
}

fn render_socket(description: &str, port: u16, name: &str) -> String {
    format!(
        "[Unit]\nDescription=Roxy {description} socket\n\n\
         [Socket]\nListenStream=0.0.0.0:{port}\nFileDescriptorName={name}\nService=roxy.service\n\n\
         [Install]\nWantedBy=sockets.target\n"
    )
}

fn render_service(executable: &Path, config_path: &Path, user: &RuntimeUser) -> String {
    format!(
        "[Unit]\nDescription=Roxy local development proxy\nAfter=network.target\n\
         Requires=roxy-http.socket roxy-https.socket\n\n\
         [Service]\nType=simple\nUser={}\nExecStart={} --config {} start --foreground\n\
         Restart=on-failure\nNoNewPrivileges=true\n\n\
         [Install]\nWantedBy=multi-user.target\n",
        user.name,
        unit_arg(&executable.display().to_string()),
        unit_arg(&config_path.display().to_string()),
    )
}

fn unit_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn run_systemctl<const N: usize>(args: [&str; N]) -> Result<()> {
    let output = Command::new("systemctl")
        .args(args)
        .output()
        .context("Failed to run systemctl")?;
    if !output.status.success() {
        bail!(
            "systemctl failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_runs_as_runtime_user() {
        let unit = render_service(
            Path::new("/usr/local/bin/roxy"),
            Path::new("/home/dev/.config/roxy/config.toml"),
            &RuntimeUser {
                name: "dev".into(),
                uid: 1000,
                gid: 1000,
                home: "/home/dev".into(),
            },
        );
        assert!(unit.contains("User=dev"));
        assert!(unit.contains("NoNewPrivileges=true"));
    }
}
