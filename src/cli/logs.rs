use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::infrastructure::tracing::default_log_path;

pub fn execute(lines: usize, clear: bool, follow: bool) -> Result<()> {
    let log_path = default_log_path();

    if clear {
        if log_path.exists() {
            fs::remove_file(&log_path).context("Failed to clear log file")?;
        }
        println!("Logs cleared.");
        return Ok(());
    }

    if !log_path.exists() {
        println!("No logs found.");
        println!("Log file: {}", log_path.display());
        println!("\nStart the daemon to generate logs: sudo roxy start");
        return Ok(());
    }

    // Show last N lines
    let content = tail_lines(&log_path, lines)?;
    if !content.is_empty() {
        print!("{}", content);
    }

    // Follow mode: keep watching for new lines
    if follow {
        tail_follow(&log_path)?;
    }

    Ok(())
}

/// Read last N lines from a file
fn tail_lines(path: &std::path::Path, n: usize) -> Result<String> {
    let content = fs::read_to_string(path).context("Failed to read log file")?;
    let all_lines: Vec<&str> = content.lines().collect();
    let start = all_lines.len().saturating_sub(n);
    let result = all_lines[start..].join("\n");
    if result.is_empty() {
        Ok(result)
    } else {
        Ok(result + "\n")
    }
}

/// Follow log file for new content (like tail -f)
fn tail_follow(path: &std::path::Path) -> Result<()> {
    let file = File::open(path).context("Failed to open log file")?;
    let mut reader = BufReader::new(file);

    // Seek to end of file
    reader.seek(SeekFrom::End(0))?;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => {
                // No new data, sleep and try again
                thread::sleep(Duration::from_millis(100));
            }
            Ok(_) => {
                // New line available
                print!("{}", line);
            }
            Err(e) => {
                return Err(e).context("Error reading log file");
            }
        }
    }
}
