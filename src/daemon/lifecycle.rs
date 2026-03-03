use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use super::Server;
use super::config_watcher::ConfigFileProvider;
use crate::domain::DomainRegistration;
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

    println!("Starting Roxy daemon...");

    // Load config fresh from disk (this path is used by the forked
    // subprocess, so it must re-read from the config file)
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let config = config_store.load()?;

    // Create reload channel for hot-reloading registrations
    let (reload_tx, reload_rx) = mpsc::channel::<Vec<DomainRegistration>>(4);

    // Create the registration provider (shared by config watcher and mgmt socket)
    let provider = Arc::new(ConfigFileProvider::new(config_path.to_path_buf()));

    let server = Server::new(
        &config,
        paths,
        reload_rx,
        reload_tx.clone(),
        provider.clone(),
    )?;
    let cancel = CancellationToken::new();

    // Spawn config file watcher for hot-reload
    let cancel_watcher = cancel.clone();
    tokio::spawn({
        let reload_tx = reload_tx;
        async move {
            provider.watch(reload_tx, cancel_watcher).await;
        }
    });

    // Spawn signal handler for graceful shutdown
    let cancel_signal = cancel.clone();
    let pid_signal = PidFile::new(paths.pid_file.clone());
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        info!("Shutdown signal received");
        let _ = pid_signal.remove();
        cancel_signal.cancel();
    });

    let result = server.run(cancel).await;

    let _ = pid_file.remove();
    result
}

/// Wait for SIGINT (Ctrl+C) or SIGTERM.
async fn wait_for_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {},
            _ = sigterm.recv() => {},
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
