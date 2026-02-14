use anyhow::{Result, bail};

use crate::domain::{DomainPattern, DomainRegistration, Route};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

use super::StepOutcome;

/// Result of a successful domain registration.
pub struct RegisterResult {
    pub registration: DomainRegistration,
    pub cert_outcome: StepOutcome,
}

/// Use case: register a new domain with routes.
pub struct RegisterDomain<'a> {
    config_store: &'a ConfigStore,
    cert_service: &'a CertificateService,
}

impl<'a> RegisterDomain<'a> {
    pub fn new(config_store: &'a ConfigStore, cert_service: &'a CertificateService) -> Self {
        Self {
            config_store,
            cert_service,
        }
    }

    /// Validate inputs, generate a certificate, and persist the registration.
    pub fn execute(&self, pattern: DomainPattern, routes: Vec<Route>) -> Result<RegisterResult> {
        if routes.is_empty() {
            bail!(
                "At least one route is required. \
                 Use --route \"/=PORT\" or --route \"/=PATH\""
            );
        }

        // ConfigStore::add_domain also rejects duplicates, but we
        // check here for a friendlier error message with guidance.
        if self.config_store.get_domain(&pattern)?.is_some() {
            bail!(
                "Domain '{}' is already registered. \
                 Use 'roxy unregister {}{}' first.",
                pattern,
                pattern.base_domain(),
                if pattern.is_wildcard() {
                    " --wildcard"
                } else {
                    ""
                }
            );
        }

        let mut registration = DomainRegistration::new(pattern.clone(), routes);

        // Generate certificate (graceful fallback)
        let cert_outcome = match self.cert_service.create_and_install(&pattern) {
            Ok(()) => {
                registration.enable_https();
                StepOutcome::Success("Certificate installed and trusted.".into())
            }
            Err(e) => StepOutcome::Warning(format!(
                "Failed to generate certificate: {}. \
                 HTTPS will not be available for this domain.",
                e
            )),
        };

        self.config_store.add_domain(registration.clone())?;

        Ok(RegisterResult {
            registration,
            cert_outcome,
        })
    }
}
