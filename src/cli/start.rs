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
        run_server(verbose, config_path, paths)
    } else {
        // Fork to background
        let exe = env::current_exe()?;

        let mut cmd = Command::new(exe);
        cmd.args([
            "--config",
            &config_path.to_string_lossy(),
            "start",
            "--foreground",
        ]);

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

#[tokio::main]
async fn run_server(verbose: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    use std::io::IsTerminal;

    use crate::daemon::Server;
    use crate::infrastructure::pid::PidFile;
    use crate::infrastructure::tracing::{TracingOutput, init_tracing};
    use tracing::info;

    // When running interactively (stdout is a TTY), log to stdout
    // When running as daemon (stdout is /dev/null), log to file
    let output = if std::io::stdout().is_terminal() {
        TracingOutput::Stdout
    } else {
        TracingOutput::File(paths.log_file.clone())
    };
    init_tracing(verbose, output);

    info!("Roxy daemon started");

    let pid_file = PidFile::new(paths.pid_file.clone());
    pid_file.write()?;

    // Handle Ctrl+C gracefully
    let pid_path_for_cleanup = paths.pid_file.clone();
    ctrlc::set_handler(move || {
        let cleanup = PidFile::new(pid_path_for_cleanup.clone());
        let _ = cleanup.remove();
        std::process::exit(0);
    })?;

    println!("Starting Roxy daemon...");

    let server = Server::new(config_path, paths)?;
    let result = server.run().await;

    pid_file.remove()?;
    result
}
