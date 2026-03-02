use std::time::Duration;

use crate::application::ports::DaemonControl;
use crate::infrastructure::pid::PidFile;

/// Adapter that bridges [`PidFile`] to the [`DaemonControl`] port.
pub struct DaemonControlAdapter<'a> {
    inner: &'a PidFile,
}

impl<'a> DaemonControlAdapter<'a> {
    pub fn new(inner: &'a PidFile) -> Self {
        Self { inner }
    }
}

impl DaemonControl for DaemonControlAdapter<'_> {
    fn is_running(&self) -> anyhow::Result<bool> {
        self.inner.is_running()
    }

    fn get_running_pid(&self) -> anyhow::Result<Option<u32>> {
        self.inner.get_running_pid()
    }

    fn stop_gracefully(&self, timeout: Duration) -> anyhow::Result<()> {
        self.inner.stop_gracefully(timeout)
    }
}
