use std::path::PathBuf;
use thiserror::Error;

pub mod generator;
pub mod service;
pub mod trust_store;

pub use generator::CertificateGenerator;
pub use service::CertificateService;

#[derive(Error, Debug)]
pub enum CertError {
    #[error("Failed to generate certificate: {0}")]
    GenerationError(String),

    #[error("Failed to write certificate to {path}: {source}")]
    WriteError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    #[error("Certificate not found for domain: {0}")]
    NotFound(String),

    #[error("Trust store operation failed: {0}")]
    TrustStoreError(String),

    #[error(
        "Permission denied. Trust store modification requires root privileges.\nRun with: sudo roxy register <domain> ..."
    )]
    PermissionDenied,
}
