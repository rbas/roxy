use std::path::Path;

use anyhow::Result;
use std::env;
use std::process::{Command, Stdio};

use crate::application::start_daemon::StartDaemon;
use crate::config::DaemonConfig;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;
use crate::infrastructure::service;

pub fn execute(
    foreground: bool,
    verbose: bool,
    config_path: &Path,
    paths: &RoxyPaths,
    daemon_config: &DaemonConfig,
) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let service = StartDaemon::new(&pid_file, daemon_config);
    let ready = service.preflight()?;

    if foreground {
        // Run in foreground (blocking) — daemon module lives in binary crate
        return crate::daemon::lifecycle::run(verbose, config_path, paths);
    }

    if service::is_installed() {
        service::activate(ready.http_port)?;
        for _ in 0..20 {
            if let Some(pid) = pid_file.get_running_pid()? {
                println!("Roxy daemon started (PID: {pid})");
                println!(
                    "Listening on 0.0.0.0:{} (HTTP) and 0.0.0.0:{} (HTTPS)",
                    ready.http_port, ready.https_port
                );
                println!("Use 'roxy stop' to stop the daemon");
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        anyhow::bail!("Roxy service was activated but did not become ready");
    }

    // Fork to background
    let exe = env::current_exe()?;

    let mut cmd = Command::new(exe);
    cmd.arg("--config")
        .arg(config_path)
        .arg("start")
        .arg("--foreground");

    // Pass verbose flag via environment to subprocess
    if verbose {
        cmd.env("ROXY_LOG", "debug");
    }

    let child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let lan_ip = get_lan_ip();
    println!("Roxy daemon started (PID: {})", child.id());
    println!(
        "Listening on 0.0.0.0:{} (HTTP) and 0.0.0.0:{} (HTTPS)",
        ready.http_port, ready.https_port
    );
    println!("LAN IP: {}", lan_ip);
    if !lan_ip.is_loopback() {
        println!("\nAccess from other devices: https://yourdomain.roxy");
    }
    println!("\nUse 'roxy status' to check status");
    println!("Use 'roxy stop' to stop the daemon");

    Ok(())
}
