use std::path::PathBuf;
use thiserror::Error;

use crate::domain::DomainPattern;

pub mod ca;
pub mod generator;
pub mod service;
pub mod trust_store;

pub use generator::CertificateGenerator;
pub use service::CertificateService;

/// Filename prefix for wildcard certificates stored on disk.
///
/// Uses underscores so it can't collide with a valid `.roxy` domain
/// (underscores are rejected by `DomainName` validation).
pub const WILDCARD_CERT_PREFIX: &str = "__wildcard__.";

/// Certificate file stem used for on-disk certificate naming.
///
/// Exact domains use the domain directly (`myapp.roxy`).
/// Wildcard domains use the `__wildcard__.` prefix
/// (`__wildcard__.myapp.roxy`).
pub fn cert_name(pattern: &DomainPattern) -> String {
    match pattern {
        DomainPattern::Exact(d) => d.as_str().to_string(),
        DomainPattern::Wildcard(d) => {
            format!("{}{}", WILDCARD_CERT_PREFIX, d.as_str())
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainName;

    #[test]
    fn exact_cert_name() {
        let name = DomainName::new("myapp.roxy").unwrap();
        let pattern = DomainPattern::Exact(name);
        assert_eq!(cert_name(&pattern), "myapp.roxy");
    }

    #[test]
    fn wildcard_cert_name() {
        let name = DomainName::new("myapp.roxy").unwrap();
        let pattern = DomainPattern::Wildcard(name);
        assert_eq!(cert_name(&pattern), "__wildcard__.myapp.roxy");
    }
}
