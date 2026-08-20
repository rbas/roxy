use std::io::IsTerminal;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use super::Server;
use crate::application::provider_registry::ProviderRegistry;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::config::watcher::ConfigFileProvider;
use crate::infrastructure::docker::DockerProvider;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;
use crate::infrastructure::tracing::{TracingOutput, init_tracing};

/// Run the Roxy daemon server.
///
/// This handles the full daemon lifecycle: tracing initialization,
/// PID file management, signal handling, and server execution.
#[tokio::main]
pub async fn run(verbose: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    // Load config before tracing so the configured log level applies to both
    // supervisor-managed and directly launched daemon processes.
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let config = config_store.load()?;

    // When running interactively (stdout is a TTY), log to stdout
    // When running as daemon (stdout is /dev/null), log to file
    let output = if std::io::stdout().is_terminal() {
        TracingOutput::Stdout
    } else {
        TracingOutput::File(paths.log_file.clone())
    };
    init_tracing(verbose, &config.daemon.log_level, output);

    info!("Roxy daemon started");

    let pid_file = PidFile::new(paths.pid_file.clone());
    pid_file.write()?;

    println!("Starting Roxy daemon...");

    // Create reload channel for hot-reloading registrations (nudge-based)
    let (reload_tx, reload_rx) = mpsc::channel::<()>(4);

    // Create the config file provider
    let config_provider = Arc::new(ConfigFileProvider::new(config_path.to_path_buf()));

    // Build the provider registry (aggregates all registration sources)
    let mut registry = ProviderRegistry::new();
    registry.add(config_provider.clone());

    // Optionally enable Docker auto-discovery
    let docker_provider = if config.docker.enabled {
        match bollard::Docker::connect_with_local_defaults() {
            Ok(docker) => {
                info!("Docker integration enabled");
                let provider = Arc::new(DockerProvider::new(docker));
                registry.add(provider.clone());
                Some(provider)
            }
            Err(e) => {
                warn!(
                    error = %e,
                    "Docker integration enabled but could not connect to Docker socket. \
                     Continuing without Docker support."
                );
                None
            }
        }
    } else {
        None
    };

    let registry = Arc::new(registry);

    let server = Server::new(&config, paths, reload_rx, reload_tx.clone(), registry)?;
    let cancel = CancellationToken::new();

    // Spawn config file watcher for hot-reload
    let cancel_watcher = cancel.clone();
    let config_reload_tx = reload_tx.clone();
    tokio::spawn(async move {
        config_provider
            .watch(config_reload_tx, cancel_watcher)
            .await;
    });

    // Spawn Docker watcher if enabled
    if let Some(provider) = docker_provider {
        let cancel_docker = cancel.clone();
        let docker_reload_tx = reload_tx;
        tokio::spawn(async move {
            crate::infrastructure::docker::watcher::watch(
                provider.docker().clone(),
                provider.state(),
                docker_reload_tx,
                cancel_docker,
            )
            .await;
        });
    }

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
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                tokio::select! {
                    _ = ctrl_c => {},
                    _ = sigterm.recv() => {},
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to register SIGTERM handler, using Ctrl+C only");
                ctrl_c.await.ok();
            }
        }
    }

    #[cfg(not(unix))]
    {
        ctrl_c.await.ok();
    }
}
