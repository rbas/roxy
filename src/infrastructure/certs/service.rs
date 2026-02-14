use super::ca::RootCA;
use super::trust_store::get_trust_store;
use super::{CertError, CertificateGenerator};
use crate::domain::DomainPattern;
use crate::infrastructure::paths::RoxyPaths;

/// High-level service for certificate operations
pub struct CertificateService {
    generator: CertificateGenerator,
    ca: RootCA,
}

impl CertificateService {
    /// Create a new CertificateService with paths from RoxyPaths
    pub fn new(paths: &RoxyPaths) -> Self {
        Self {
            generator: CertificateGenerator::new(paths.data_dir.clone(), paths.certs_dir.clone()),
            ca: RootCA::new(paths.data_dir.clone()),
        }
    }

    /// Initialize the Root CA (called during `roxy install`)
    pub fn init_ca(&self) -> Result<(), CertError> {
        if self.ca.exists() {
            return Ok(());
        }

        // Generate CA
        self.ca.generate()?;

        // Add CA to trust store
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

    /// Generate a certificate for a domain pattern (signed by the Root CA).
    ///
    /// For exact patterns, generates a single-domain cert.
    /// For wildcard patterns, generates a cert with SANs for base + *.base.
    pub fn create_and_install(&self, pattern: &DomainPattern) -> Result<(), CertError> {
        if !self.ca.exists() {
            return Err(CertError::GenerationError(
                "Root CA not found. Run 'sudo roxy install' first.".to_string(),
            ));
        }

        let cert = self.generator.generate(pattern)?;
        self.generator.save(&cert)?;
        Ok(())
    }

    /// Remove certificate files for a domain pattern.
    pub fn remove(&self, pattern: &DomainPattern) -> Result<(), CertError> {
        self.generator.delete(pattern)
    }

    /// Check if certificate exists for a domain pattern.
    pub fn exists(&self, pattern: &DomainPattern) -> bool {
        self.generator.exists(pattern)
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
