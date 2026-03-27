use crate::domain::DomainRegistration;

#[derive(Debug, Clone)]
#[allow(dead_code)] // Used by trait impls; no production caller for status() yet.
pub struct DaemonRuntimeInfo {
    pub pid: u32,
    pub registrations: Vec<DomainRegistration>,
    pub http_port: u16,
    pub https_port: u16,
    pub dns_port: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum DaemonConnectionError {
    #[error("Daemon is not running")]
    NotRunning,
    #[error("Connection failed: {0}")]
    ConnectionFailed(#[source] anyhow::Error),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

/// Port for communicating with a running daemon process.
/// Complements DaemonControl (lifecycle) with runtime queries.
#[allow(dead_code)] // status()/reload() have impls but no production callers yet.
pub trait DaemonConnection {
    fn status(&self) -> Result<DaemonRuntimeInfo, DaemonConnectionError>;
    fn reload(&self) -> Result<(), DaemonConnectionError>;
    fn list_registrations(&self) -> Result<Vec<DomainRegistration>, DaemonConnectionError>;
}
