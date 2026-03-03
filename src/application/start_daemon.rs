use anyhow::{Result, bail};

use crate::config::DaemonConfig;

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
    config: &'a DaemonConfig,
}

impl<'a> StartDaemon<'a> {
    pub fn new(daemon: &'a dyn DaemonControl, config: &'a DaemonConfig) -> Self {
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
            .map_err(|msg| anyhow::anyhow!("Configuration validation failed: {}", msg))?;

        Ok(StartReady {
            http_port: self.config.http_port,
            https_port: self.config.https_port,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    #[test]
    fn preflight_succeeds_when_daemon_stopped() {
        let daemon = InMemoryDaemonControl::stopped();
        let config = DaemonConfig::default();
        let svc = StartDaemon::new(&daemon, &config);

        let ready = svc.preflight().unwrap();
        assert_eq!(ready.http_port, 80);
        assert_eq!(ready.https_port, 443);
    }

    #[test]
    fn preflight_fails_when_daemon_already_running() {
        let daemon = InMemoryDaemonControl::running(1234);
        let config = DaemonConfig::default();
        let svc = StartDaemon::new(&daemon, &config);

        let err = svc.preflight().err().unwrap();
        assert!(err.to_string().contains("already running"));
        assert!(err.to_string().contains("1234"));
    }

    #[test]
    fn preflight_validates_config() {
        let daemon = InMemoryDaemonControl::stopped();
        let config = DaemonConfig {
            http_port: 0,
            ..DaemonConfig::default()
        };
        let svc = StartDaemon::new(&daemon, &config);

        let err = svc.preflight().err().unwrap();
        assert!(err.to_string().contains("http_port cannot be 0"));
    }
}
