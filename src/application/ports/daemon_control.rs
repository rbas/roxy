use std::time::Duration;

/// Port for daemon process lifecycle operations.
pub trait DaemonControl {
    /// Check if the daemon process is currently running.
    fn is_running(&self) -> anyhow::Result<bool>;

    /// Get the PID of the running daemon, if any.
    fn get_running_pid(&self) -> anyhow::Result<Option<u32>>;

    /// Stop the daemon gracefully with the given timeout.
    fn stop_gracefully(&self, timeout: Duration) -> anyhow::Result<()>;
}
