use super::ca::RootCA;
use super::trust_store::{TrustStore, get_trust_store};
use super::{CertError, CertificateGenerator};
use crate::domain::DomainName;

/// High-level service for certificate operations
pub struct CertificateService {
    generator: CertificateGenerator,
    ca: RootCA,
}

impl CertificateService {
    pub fn new() -> Self {
        Self {
            generator: CertificateGenerator::new(),
            ca: RootCA::new(),
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

    /// Generate a certificate for a domain (signed by the Root CA)
    /// The certificate is automatically trusted because the CA is trusted
    pub fn create_and_install(&self, domain: &DomainName) -> Result<(), CertError> {
        // Ensure CA exists
        if !self.ca.exists() {
            return Err(CertError::GenerationError(
                "Root CA not found. Run 'sudo roxy install' first.".to_string(),
            ));
        }

        // Generate certificate (signed by CA)
        let cert = self.generator.generate(domain)?;

        // Save to disk (no need to add to trust store - CA is already trusted)
        self.generator.save(&cert)?;

        Ok(())
    }

    /// Remove certificate files for a domain
    /// Note: No trust store removal needed since we use CA-based trust
    pub fn remove(&self, domain: &DomainName) -> Result<(), CertError> {
        // Delete certificate files
        self.generator.delete(domain)?;

        Ok(())
    }

    /// Check if certificate exists for a domain
    pub fn exists(&self, domain: &DomainName) -> bool {
        self.generator.exists(domain)
    }

    /// Check if certificate is trusted (CA is trusted = all certs trusted)
    pub fn is_trusted(&self, _domain: &DomainName) -> Result<bool, CertError> {
        // If CA is trusted, all certs signed by it are trusted
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

impl Default for CertificateService {
    fn default() -> Self {
        Self::new()
    }
}
