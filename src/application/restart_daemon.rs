use std::time::Duration;

use anyhow::{Result, bail};

use crate::infrastructure::config::Config;
use crate::infrastructure::paths::RoxyPaths;

use super::ports::{ConfigLoader, DaemonControl};

/// Fresh config + paths ready for starting the daemon after a stop.
pub struct RestartReady {
    pub config: Config,
    pub paths: RoxyPaths,
}

/// Application service for restarting/reloading the daemon.
///
/// Both `restart` and `reload` CLI commands use this.
/// The `require_running` parameter distinguishes them:
///   - `reload` requires the daemon to be running
///   - `restart` tolerates a stopped daemon
pub struct RestartDaemon<'a> {
    daemon: &'a dyn DaemonControl,
    config_loader: &'a dyn ConfigLoader,
}

impl<'a> RestartDaemon<'a> {
    pub fn new(daemon: &'a dyn DaemonControl, config_loader: &'a dyn ConfigLoader) -> Self {
        Self {
            daemon,
            config_loader,
        }
    }

    /// Stop the current daemon (if running) and return fresh config for restarting.
    ///
    /// When `require_running` is true, returns an error if the daemon is not running.
    pub fn execute(&self, require_running: bool) -> Result<RestartReady> {
        let is_running = self.daemon.is_running()?;

        if require_running && !is_running {
            bail!("Roxy daemon is not running.\nStart it with: sudo roxy start");
        }

        if is_running {
            self.daemon.stop_gracefully(Duration::from_millis(500))?;
            // Brief pause to ensure clean shutdown
            std::thread::sleep(Duration::from_millis(500));
        }

        // Re-load config from disk to pick up changes
        let fresh_config = self.config_loader.load()?;
        let fresh_paths = fresh_config.paths.clone();

        Ok(RestartReady {
            config: fresh_config,
            paths: fresh_paths,
        })
    }
}
