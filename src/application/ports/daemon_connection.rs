// Port types for CLI-to-daemon communication. Used by testkit now;
// concrete Unix-socket adapter comes when CLI commands are wired up.
#![allow(dead_code)]

use crate::domain::DomainRegistration;

#[derive(Debug, Clone)]
pub struct DaemonStatus {
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
pub trait DaemonConnection {
    fn status(&self) -> Result<DaemonStatus, DaemonConnectionError>;
    fn reload(&self) -> Result<(), DaemonConnectionError>;
    fn list_registrations(&self) -> Result<Vec<DomainRegistration>, DaemonConnectionError>;
}
