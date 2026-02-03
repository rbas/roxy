use super::trust_store::{get_trust_store, TrustStore};
use super::{CertError, CertificateGenerator};
use crate::domain::DomainName;

/// High-level service for certificate operations
pub struct CertificateService {
    generator: CertificateGenerator,
}

impl CertificateService {
    pub fn new() -> Self {
        Self {
            generator: CertificateGenerator::new(),
        }
    }

    /// Generate and install a certificate for a domain
    pub fn create_and_install(&self, domain: &DomainName) -> Result<(), CertError> {
        // Generate certificate
        let cert = self.generator.generate(domain)?;

        // Save to disk
        let paths = self.generator.save(&cert)?;

        // Add to trust store
        let trust_store = get_trust_store()?;
        trust_store.add_certificate(&paths.cert, domain)?;

        Ok(())
    }

    /// Remove certificate from trust store and delete files
    pub fn remove(&self, domain: &DomainName) -> Result<(), CertError> {
        // Remove from trust store first
        let trust_store = get_trust_store()?;
        trust_store.remove_certificate(domain)?;

        // Delete certificate files
        self.generator.delete(domain)?;

        Ok(())
    }

    /// Check if certificate exists for a domain
    pub fn exists(&self, domain: &DomainName) -> bool {
        self.generator.exists(domain)
    }

    /// Check if certificate is trusted in system
    pub fn is_trusted(&self, domain: &DomainName) -> Result<bool, CertError> {
        let trust_store = get_trust_store()?;
        trust_store.is_trusted(domain)
    }
}

impl Default for CertificateService {
    fn default() -> Self {
        Self::new()
    }
}
