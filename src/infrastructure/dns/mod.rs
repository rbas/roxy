use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DnsError {
    #[error(
        "Permission denied. DNS configuration requires root privileges.\nRun with: sudo roxy install"
    )]
    PermissionDenied,

    #[error("Failed to write DNS configuration to {path}: {source}")]
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to remove DNS configuration from {path}: {source}")]
    RemoveError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("DNS validation failed: {0}")]
    ValidationFailed(String),

    #[error("Unsupported platform: {0}")]
    #[allow(dead_code)] // Used only on non-macOS platforms
    UnsupportedPlatform(String),
}

pub trait DnsService {
    /// Configure wildcard DNS for *.roxy → 127.0.0.1
    /// The port parameter specifies which port the DNS server listens on
    fn setup(&self, port: u16) -> Result<(), DnsError>;

    /// Remove DNS configuration
    fn cleanup(&self) -> Result<(), DnsError>;

    /// Validate DNS is working correctly
    fn validate(&self) -> Result<(), DnsError>;

    /// Check if DNS is already configured
    fn is_configured(&self) -> bool;
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOsDnsService;

/// Get the DNS service for the current platform
pub fn get_dns_service() -> Result<impl DnsService, DnsError> {
    #[cfg(target_os = "macos")]
    {
        Ok(MacOsDnsService::new())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err(DnsError::UnsupportedPlatform(
            std::env::consts::OS.to_string(),
        ))
    }
}
