use std::fs::{self, OpenOptions};
use std::path::PathBuf;

use tracing_subscriber::{EnvFilter, fmt, prelude::*};

/// Output destination for tracing
pub enum TracingOutput {
    /// Output to stdout (for foreground/development mode)
    Stdout,
    /// Output to a log file (for daemon mode)
    File(PathBuf),
}

/// Returns the default log file path: ~/.roxy/logs/roxy.log
pub fn default_log_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".roxy")
        .join("logs")
        .join("roxy.log")
}

/// Initialize tracing based on configuration
/// Priority: ROXY_LOG env > verbose flag > default (info)
pub fn init_tracing(verbose: bool, output: TracingOutput) {
    let filter = EnvFilter::try_from_env("ROXY_LOG").unwrap_or_else(|_| {
        let level = if verbose { "debug" } else { "info" };
        EnvFilter::new(format!("roxy={}", level))
    });

    match output {
        TracingOutput::Stdout => {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().with_target(false))
                .init();
        }
        TracingOutput::File(path) => {
            // Ensure log directory exists
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }

            // Open file for appending
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .expect("Failed to open log file");

            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(false)
                        .with_ansi(false)
                        .with_writer(file),
                )
                .init();
        }
    }
}
