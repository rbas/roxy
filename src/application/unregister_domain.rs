use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, DomainRegistration};

use super::StepOutcome;
use super::ports::{CertificateManager, DomainRepository};

/// Result of a successful domain unregistration.
pub struct UnregisterResult {
    pub registration: DomainRegistration,
    pub cert_outcome: StepOutcome,
}

/// Use case: unregister a domain and clean up its certificate.
pub struct UnregisterDomain<'a> {
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
}

impl<'a> UnregisterDomain<'a> {
    pub fn new(domains: &'a dyn DomainRepository, certs: &'a dyn CertificateManager) -> Self {
        Self { domains, certs }
    }

    /// Look up the registration so the CLI can show a confirmation
    /// prompt before proceeding with `execute()`.
    pub fn preview(&self, pattern: &DomainPattern) -> Result<DomainRegistration> {
        self.domains
            .get(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' is not registered.", pattern))
    }

    /// Remove the domain certificate and config entry.
    pub fn execute(&self, pattern: &DomainPattern) -> Result<UnregisterResult> {
        let registration = self.preview(pattern)?;

        let cert_outcome = if self.certs.exists(pattern) {
            match self.certs.remove(pattern) {
                Ok(()) => StepOutcome::Success("Certificate removed.".into()),
                Err(e) => StepOutcome::Warning(format!("Failed to remove certificate: {}", e)),
            }
        } else {
            StepOutcome::Skipped("No certificate to remove.".into())
        };

        self.domains.remove(pattern)?;

        Ok(UnregisterResult {
            registration,
            cert_outcome,
        })
    }
}
