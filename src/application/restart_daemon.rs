use std::time::Duration;

use anyhow::{Result, bail};

use crate::config::{DaemonConfig, RoxyPaths};

use super::ports::{ConfigLoader, DaemonControl};

/// Fresh daemon config + paths ready for starting the daemon after a stop.
pub struct RestartReady {
    pub daemon_config: DaemonConfig,
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

    /// Restart the daemon. Tolerates a stopped daemon (just re-reads config).
    pub fn restart(&self) -> Result<RestartReady> {
        self.stop_and_reload(false)
    }

    /// Reload the daemon. Fails if the daemon is not running.
    pub fn reload(&self) -> Result<RestartReady> {
        self.stop_and_reload(true)
    }

    fn stop_and_reload(&self, require_running: bool) -> Result<RestartReady> {
        let is_running = self.daemon.is_running()?;

        if require_running && !is_running {
            bail!("Roxy daemon is not running.\nStart it with: sudo roxy start");
        }

        if is_running {
            self.daemon.stop_gracefully(Duration::from_millis(500))?;
        }

        // Re-load config from disk to pick up changes
        let (daemon_config, paths) = self.config_loader.load()?;

        Ok(RestartReady {
            daemon_config,
            paths,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    #[test]
    fn restarts_running_daemon() {
        let daemon = InMemoryDaemonControl::running(1234);
        let loader = InMemoryConfigLoader::existing();
        let svc = RestartDaemon::new(&daemon, &loader);

        let ready = svc.restart().unwrap();

        // Daemon was stopped
        assert!(!daemon.is_running().unwrap());
        // Returns fresh config for re-start
        assert_eq!(ready.daemon_config.http_port, 80);
    }

    #[test]
    fn restart_tolerates_stopped_daemon() {
        let daemon = InMemoryDaemonControl::stopped();
        let loader = InMemoryConfigLoader::existing();
        let svc = RestartDaemon::new(&daemon, &loader);

        let ready = svc.restart().unwrap();
        assert_eq!(ready.daemon_config.http_port, 80);
    }

    #[test]
    fn reload_fails_when_daemon_not_running() {
        let daemon = InMemoryDaemonControl::stopped();
        let loader = InMemoryConfigLoader::existing();
        let svc = RestartDaemon::new(&daemon, &loader);

        let err = svc.reload().err().unwrap();
        assert!(err.to_string().contains("not running"));
    }

    #[test]
    fn reload_succeeds_when_daemon_running() {
        let daemon = InMemoryDaemonControl::running(5678);
        let loader = InMemoryConfigLoader::existing();
        let svc = RestartDaemon::new(&daemon, &loader);

        let ready = svc.reload().unwrap();

        assert!(!daemon.is_running().unwrap());
        assert_eq!(ready.daemon_config.dns_port, 1053);
    }
}
