use std::path::Path;

use anyhow::{Context, Result, bail};
use std::env;
use std::process::{Command, Stdio};

use crate::infrastructure::config::Config;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(
    foreground: bool,
    verbose: bool,
    config_path: &Path,
    paths: &RoxyPaths,
    config: &Config,
) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());

    // Check if already running
    if let Some(pid) = pid_file.get_running_pid()? {
        bail!(
            "Roxy daemon is already running (PID: {})\nUse 'roxy stop' to stop it first.",
            pid
        );
    }

    // Validate configuration before starting
    config
        .validate()
        .context("Configuration validation failed")?;

    if foreground {
        // Run in foreground (blocking)
        crate::daemon::lifecycle::run(verbose, config_path, paths)
    } else {
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
            config.daemon.http_port, config.daemon.https_port
        );
        println!("LAN IP: {}", lan_ip);
        if !lan_ip.is_loopback() {
            println!("\nAccess from other devices: https://yourdomain.roxy");
        }
        println!("\nUse 'roxy status' to check status");
        println!("Use 'roxy stop' to stop the daemon");
        println!(
            "\nHeads up: Roxy is still finding her feet (v{}).",
            env!("CARGO_PKG_VERSION")
        );
        println!("Things may shift around. If something bites, let me know!");
        println!("https://github.com/rbas/roxy/issues");

        Ok(())
    }
}
