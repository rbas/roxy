#[derive(Debug, thiserror::Error)]
pub enum DnsConfigError {
    #[error(
        "Permission denied. DNS configuration requires root privileges.\n\
         Run with: sudo roxy install"
    )]
    PermissionDenied,

    #[error("DNS validation failed: {0}")]
    ValidationFailed(String),

    #[error("Unsupported platform: {0}")]
    UnsupportedPlatform(String),

    #[error("{0}")]
    OperationFailed(#[source] anyhow::Error),
}

/// Port for DNS resolver configuration.
pub trait DnsManager {
    /// Configure wildcard DNS for *.roxy domains.
    fn setup(&self, port: u16) -> Result<(), DnsConfigError>;

    /// Remove DNS configuration.
    fn cleanup(&self) -> Result<(), DnsConfigError>;

    /// Validate DNS is working correctly.
    fn validate(&self) -> Result<(), DnsConfigError>;

    /// Check if DNS is already configured.
    fn is_configured(&self) -> bool;
}
