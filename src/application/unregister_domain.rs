use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, DomainRegistration};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

use super::StepOutcome;

/// Result of a successful domain unregistration.
pub struct UnregisterResult {
    pub registration: DomainRegistration,
    pub cert_outcome: StepOutcome,
}

/// Use case: unregister a domain and clean up its certificate.
pub struct UnregisterDomain<'a> {
    config_store: &'a ConfigStore,
    cert_service: &'a CertificateService,
}

impl<'a> UnregisterDomain<'a> {
    pub fn new(config_store: &'a ConfigStore, cert_service: &'a CertificateService) -> Self {
        Self {
            config_store,
            cert_service,
        }
    }

    /// Look up the registration so the CLI can show a confirmation
    /// prompt before proceeding with `execute()`.
    pub fn preview(&self, pattern: &DomainPattern) -> Result<DomainRegistration> {
        self.config_store
            .get_domain(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' is not registered.", pattern))
    }

    /// Remove the domain certificate and config entry.
    pub fn execute(&self, pattern: &DomainPattern) -> Result<UnregisterResult> {
        let registration = self.preview(pattern)?;

        let cert_outcome = if self.cert_service.exists(pattern) {
            match self.cert_service.remove(pattern) {
                Ok(()) => StepOutcome::Success("Certificate removed.".into()),
                Err(e) => StepOutcome::Warning(format!("Failed to remove certificate: {}", e)),
            }
        } else {
            StepOutcome::Skipped("No certificate to remove.".into())
        };

        self.config_store.remove_domain(pattern)?;

        Ok(UnregisterResult {
            registration,
            cert_outcome,
        })
    }
}
