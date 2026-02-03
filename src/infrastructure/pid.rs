use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process;

pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    pub fn new() -> Self {
        let path = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".roxy")
            .join("roxy.pid");
        Self { path }
    }

    /// Write current process PID to file
    pub fn write(&self) -> Result<()> {
        let pid = process::id();
        fs::create_dir_all(self.path.parent().unwrap())?;
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

impl Default for PidFile {
    fn default() -> Self {
        Self::new()
    }
}
