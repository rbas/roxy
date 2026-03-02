use std::time::Duration;

use anyhow::Result;

/// Platform-agnostic process control operations.
pub trait ProcessControl: Send {
    /// Check whether a process with the given PID exists.
    fn process_exists(&self, pid: u32) -> bool;

    /// Terminate a process gracefully, escalating to a forced kill after `timeout`.
    fn terminate(&self, pid: u32, timeout: Duration) -> Result<()>;
}

#[cfg(unix)]
mod unix;

/// Get the process control provider for the current platform.
#[cfg(unix)]
pub fn get_process_control() -> Box<dyn ProcessControl> {
    Box::new(unix::UnixProcessControl)
}

/// Get the process control provider for the current platform.
#[cfg(not(unix))]
pub fn get_process_control() -> Box<dyn ProcessControl> {
    Box::new(UnsupportedProcessControl)
}

/// Fallback for unsupported platforms.
#[cfg(not(unix))]
struct UnsupportedProcessControl;

#[cfg(not(unix))]
impl ProcessControl for UnsupportedProcessControl {
    fn process_exists(&self, _pid: u32) -> bool {
        false
    }

    fn terminate(&self, _pid: u32, _timeout: Duration) -> Result<()> {
        anyhow::bail!(
            "Process control not implemented for {}. \
             Cannot manage daemon processes on this platform.",
            std::env::consts::OS
        )
    }
}
