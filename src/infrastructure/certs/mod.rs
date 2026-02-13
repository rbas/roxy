use std::path::PathBuf;
use thiserror::Error;

pub mod ca;
pub mod generator;
pub mod service;
pub mod trust_store;

pub use generator::CertificateGenerator;
pub use service::CertificateService;

/// Filename prefix for wildcard certificates stored in `certs_dir`.
///
/// We intentionally use underscores so it can't collide with a valid `.roxy`
/// domain (underscores are rejected by `DomainName` validation).
pub const WILDCARD_CERT_PREFIX: &str = "__wildcard__.";

#[derive(Error, Debug)]
pub enum CertError {
    #[error("Failed to generate certificate: {0}")]
    GenerationError(String),

    #[error("Failed to write certificate to {path}: {source}")]
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to read certificate from {path}: {source}")]
    ReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to delete certificate at {path}: {source}")]
    DeleteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Trust store operation failed: {0}")]
    TrustStoreError(String),

    #[error(
        "Permission denied. Trust store modification requires root privileges.\nRun with: sudo roxy register <domain> ..."
    )]
    PermissionDenied,
}
