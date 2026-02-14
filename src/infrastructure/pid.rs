use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process;
use std::time::Duration;

pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Write current process PID to file
    pub fn write(&self) -> Result<()> {
        let pid = process::id();
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, pid.to_string()).context("Failed to write PID file")?;
        Ok(())
    }

    /// Read PID from file
    pub fn read(&self) -> Result<Option<u32>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&self.path).context("Failed to read PID file")?;
        let pid: u32 = content.trim().parse().context("Invalid PID in file")?;
        Ok(Some(pid))
    }

    /// Check if process with stored PID is running
    pub fn is_running(&self) -> Result<bool> {
        match self.read()? {
            Some(pid) => Ok(process_exists(pid)),
            None => Ok(false),
        }
    }

    /// Remove PID file
    pub fn remove(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path).context("Failed to remove PID file")?;
        }
        Ok(())
    }

    /// Get stored PID if daemon is running
    pub fn get_running_pid(&self) -> Result<Option<u32>> {
        match self.read()? {
            Some(pid) if process_exists(pid) => Ok(Some(pid)),
            _ => Ok(None),
        }
    }

    /// Stop the running daemon gracefully.
    ///
    /// Sends SIGTERM, waits for the given timeout, then sends SIGKILL
    /// if the process is still running. Removes the PID file on success.
    pub fn stop_gracefully(&self, timeout: Duration) -> Result<()> {
        let pid = match self.get_running_pid()? {
            Some(pid) => pid,
            None => return Ok(()),
        };

        terminate_process(pid, timeout)?;
        self.remove()
    }
}

/// Send SIGTERM, wait, then SIGKILL if still running.
#[cfg(unix)]
fn terminate_process(pid: u32, timeout: Duration) -> Result<()> {
    use std::process::Command;

    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()?;

    std::thread::sleep(timeout);

    if process_exists(pid) {
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .output()?;
    }

    Ok(())
}

/// Check if a process exists (Unix-specific)
#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    use std::process::Command;
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
