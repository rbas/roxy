use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use anyhow::{Context, Result};

pub struct LogFile {
    path: PathBuf,
}

impl LogFile {
    pub fn new() -> Self {
        let path = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".roxy")
            .join("logs")
            .join("roxy.log");
        Self { path }
    }

    fn ensure_dir(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(())
    }

    /// Append a log message with timestamp
    pub fn log(&self, message: &str) -> Result<()> {
        self.ensure_dir()?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .context("Failed to open log file")?;

        let timestamp = humantime::format_rfc3339_seconds(SystemTime::now());
        writeln!(file, "[{}] {}", timestamp, message)?;
        Ok(())
    }

    /// Read last N lines from log file
    pub fn tail(&self, lines: usize) -> Result<String> {
        if !self.path.exists() {
            return Ok(String::new());
        }

        let content = fs::read_to_string(&self.path).context("Failed to read log file")?;

        let all_lines: Vec<&str> = content.lines().collect();
        let start = all_lines.len().saturating_sub(lines);
        Ok(all_lines[start..].join("\n"))
    }

    /// Clear log file
    pub fn clear(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path).context("Failed to clear log file")?;
        }
        Ok(())
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Default for LogFile {
    fn default() -> Self {
        Self::new()
    }
}
