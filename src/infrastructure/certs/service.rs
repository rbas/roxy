use super::CertError;
use super::ca::RootCA;
use super::trust_store::{TrustStore, get_trust_store};
use crate::application::ports::{CertificateError, CertificateManager};
use crate::infrastructure::paths::RoxyPaths;

/// High-level service for certificate operations
pub struct CertificateService {
    ca: RootCA,
}

impl CertificateService {
    /// Create a new CertificateService with paths from RoxyPaths
    pub fn new(paths: &RoxyPaths) -> Self {
        Self {
            ca: RootCA::new(paths.data_dir.clone()),
        }
    }

    /// Initialize the Root CA (called during `roxy install`)
    pub fn init_ca(&self) -> Result<(), CertError> {
        if !self.ca.exists() {
            self.ca.generate()?;
        }

        // Trust installation is idempotent and is the only privileged
        // certificate operation Roxy performs.
        let trust_store = get_trust_store()?;
        trust_store.add_ca(&self.ca.cert_path())?;

        Ok(())
    }

    /// Check if the Root CA exists and is trusted
    pub fn is_ca_installed(&self) -> Result<bool, CertError> {
        if !self.ca.exists() {
            return Ok(false);
        }

        let trust_store = get_trust_store()?;
        trust_store.is_ca_trusted()
    }

    /// Check if certificate is trusted (CA is trusted = all certs trusted)
    pub fn is_trusted(&self) -> Result<bool, CertError> {
        self.is_ca_installed()
    }

    /// Remove the Root CA (for uninstall)
    pub fn remove_ca(&self) -> Result<(), CertError> {
        let trust_store = get_trust_store()?;
        trust_store.remove_ca()?;
        self.ca.delete()?;
        Ok(())
    }
}

impl CertificateManager for CertificateService {
    fn init_ca(&self) -> Result<(), CertificateError> {
        CertificateService::init_ca(self).map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn is_ca_installed(&self) -> Result<bool, CertificateError> {
        CertificateService::is_ca_installed(self)
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn remove_ca(&self) -> Result<(), CertificateError> {
        CertificateService::remove_ca(self).map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn is_trusted(&self) -> Result<bool, CertificateError> {
        CertificateService::is_trusted(self)
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }
}
