use std::io::IsTerminal;
use std::path::Path;

use anyhow::Result;
use tracing::info;

use super::Server;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;
use crate::infrastructure::tracing::{TracingOutput, init_tracing};

/// Run the Roxy daemon server.
///
/// This handles the full daemon lifecycle: tracing initialization,
/// PID file management, signal handling, and server execution.
#[tokio::main]
pub async fn run(verbose: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
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
    let cleanup_pid = PidFile::new(paths.pid_file.clone());
    ctrlc::set_handler(move || {
        let _ = cleanup_pid.remove();
        std::process::exit(0);
    })?;

    println!("Starting Roxy daemon...");

    // Load config fresh from disk (this path is used by the forked
    // subprocess, so it must re-read from the config file)
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let config = config_store.load()?;

    let server = Server::new(&config, paths)?;
    let result = server.run().await;

    pid_file.remove()?;
    result
}
