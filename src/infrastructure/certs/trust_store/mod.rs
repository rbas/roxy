use std::path::Path;

use super::CertError;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOsTrustStore;

/// Trait for platform-specific trust store operations (CA-based trust)
pub trait TrustStore {
    /// Add the Root CA to the system trust store
    fn add_ca(&self, cert_path: &Path) -> Result<(), CertError>;

    /// Remove the Root CA from the system trust store
    fn remove_ca(&self) -> Result<(), CertError>;

    /// Check if the Root CA is trusted
    fn is_ca_trusted(&self) -> Result<bool, CertError>;
}

/// Get the trust store for the current platform
#[cfg(target_os = "macos")]
pub fn get_trust_store() -> Result<Box<dyn TrustStore>, CertError> {
    Ok(Box::new(MacOsTrustStore::new()))
}

#[cfg(not(target_os = "macos"))]
pub fn get_trust_store() -> Result<Box<dyn TrustStore>, CertError> {
    Err(CertError::TrustStoreError(format!(
        "Unsupported platform: {}",
        std::env::consts::OS
    )))
}
