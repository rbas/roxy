use anyhow::{Context, Result, bail};

use crate::infrastructure::config::Config;

use super::ports::DaemonControl;

/// Validated preconditions for starting the daemon.
pub struct StartReady {
    pub http_port: u16,
    pub https_port: u16,
}

/// Application service for validating daemon start preconditions.
///
/// The actual daemon execution (foreground or forked background) stays in
/// the CLI layer, which has access to the `daemon` binary-crate module.
pub struct StartDaemon<'a> {
    daemon: &'a dyn DaemonControl,
    config: &'a Config,
}

impl<'a> StartDaemon<'a> {
    pub fn new(daemon: &'a dyn DaemonControl, config: &'a Config) -> Self {
        Self { daemon, config }
    }

    /// Validate preconditions for starting the daemon.
    ///
    /// Checks that the daemon is not already running and that the config
    /// is valid. Returns the ports that will be used.
    pub fn preflight(&self) -> Result<StartReady> {
        if let Some(pid) = self.daemon.get_running_pid()? {
            bail!(
                "Roxy daemon is already running (PID: {})\nUse 'roxy stop' to stop it first.",
                pid
            );
        }

        self.config
            .validate()
            .context("Configuration validation failed")?;

        Ok(StartReady {
            http_port: self.config.daemon.http_port,
            https_port: self.config.daemon.https_port,
        })
    }
}
