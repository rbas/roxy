use anyhow::{Context, Result, bail};
use std::env;
use std::process::{Command, Stdio};

use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::logging::LogFile;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::pid::PidFile;

pub fn execute(foreground: bool) -> Result<()> {
    let pid_file = PidFile::new();

    // Check if already running
    if let Some(pid) = pid_file.get_running_pid()? {
        bail!(
            "Roxy daemon is already running (PID: {})\nUse 'roxy stop' to stop it first.",
            pid
        );
    }

    // Validate configuration before starting
    let config_store = ConfigStore::new();
    let config = config_store
        .load()
        .context("Failed to load configuration")?;

    config
        .validate()
        .context("Configuration validation failed")?;

    if foreground {
        // Run in foreground (blocking)
        run_server()
    } else {
        // Fork to background
        let exe = env::current_exe()?;

        let child = Command::new(exe)
            .args(["start", "--foreground"])
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
            println!("\nAccess from Docker/other devices: https://yourdomain.roxy");
        }
        println!("\nUse 'roxy status' to check status");
        println!("Use 'roxy stop' to stop the daemon");

        Ok(())
    }
}

#[tokio::main]
async fn run_server() -> Result<()> {
    use crate::daemon::Server;
    use crate::infrastructure::pid::PidFile;

    let log = LogFile::new();
    let _ = log.log("Roxy daemon started");

    let pid_file = PidFile::new();
    pid_file.write()?;

    // Handle Ctrl+C gracefully
    let pid_file_cleanup = PidFile::new();
    ctrlc::set_handler(move || {
        let _ = pid_file_cleanup.remove();
        std::process::exit(0);
    })?;

    println!("Starting Roxy daemon...");

    let server = Server::new()?;
    let result = server.run().await;

    pid_file.remove()?;
    result
}
