use std::path::Path;

use super::CertError;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::MacOsTrustStore;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::LinuxTrustStore;

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
pub fn get_trust_store() -> Result<impl TrustStore, CertError> {
    Ok(MacOsTrustStore::new())
}

#[cfg(target_os = "linux")]
pub fn get_trust_store() -> Result<impl TrustStore, CertError> {
    Ok(LinuxTrustStore::new())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn get_trust_store() -> Result<impl TrustStore, CertError> {
    Err::<UnsupportedTrustStore, _>(CertError::TrustStoreError(format!(
        "Unsupported platform: {}",
        std::env::consts::OS
    )))
}

/// Fallback for unsupported platforms — never constructed.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
struct UnsupportedTrustStore;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl TrustStore for UnsupportedTrustStore {
    fn add_ca(&self, _cert_path: &Path) -> Result<(), CertError> {
        Err(CertError::TrustStoreError(format!(
            "Unsupported platform: {}",
            std::env::consts::OS
        )))
    }

    fn remove_ca(&self) -> Result<(), CertError> {
        Err(CertError::TrustStoreError(format!(
            "Unsupported platform: {}",
            std::env::consts::OS
        )))
    }

    fn is_ca_trusted(&self) -> Result<bool, CertError> {
        Err(CertError::TrustStoreError(format!(
            "Unsupported platform: {}",
            std::env::consts::OS
        )))
    }
}
