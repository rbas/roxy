use std::time::Duration;

use anyhow::{Result, bail};

use super::ports::DaemonControl;

/// Application service for stopping the daemon.
pub struct StopDaemon<'a> {
    daemon: &'a dyn DaemonControl,
}

impl<'a> StopDaemon<'a> {
    pub fn new(daemon: &'a dyn DaemonControl) -> Self {
        Self { daemon }
    }

    /// Stop the daemon, or bail if it is not running.
    pub fn execute(&self) -> Result<()> {
        if self.daemon.get_running_pid()?.is_none() {
            bail!("Roxy daemon is not running.");
        }

        self.daemon.stop_gracefully(Duration::from_millis(500))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    #[test]
    fn stops_running_daemon() {
        let daemon = InMemoryDaemonControl::running(1234);
        let svc = StopDaemon::new(&daemon);

        svc.execute().unwrap();

        assert!(!daemon.is_running().unwrap());
    }

    #[test]
    fn fails_when_daemon_not_running() {
        let daemon = InMemoryDaemonControl::stopped();
        let svc = StopDaemon::new(&daemon);

        let err = svc.execute().err().unwrap();
        assert!(err.to_string().contains("not running"));
    }
}
