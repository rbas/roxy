use anyhow::{Result, bail};

use crate::domain::{DomainPattern, DomainRegistration, Route};

use super::StepOutcome;
use super::ports::{CertificateManager, DomainRepository};

/// Result of a successful domain registration.
pub struct RegisterResult {
    pub registration: DomainRegistration,
    pub cert_outcome: StepOutcome,
}

/// Use case: register a new domain with routes.
pub struct RegisterDomain<'a> {
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
}

impl<'a> RegisterDomain<'a> {
    pub fn new(domains: &'a dyn DomainRepository, certs: &'a dyn CertificateManager) -> Self {
        Self { domains, certs }
    }

    /// Validate inputs, generate a certificate, and persist the registration.
    pub fn execute(&self, pattern: DomainPattern, routes: Vec<Route>) -> Result<RegisterResult> {
        if routes.is_empty() {
            bail!(
                "At least one route is required. \
                 Use --route \"/=PORT\" or --route \"/=PATH\""
            );
        }

        // DomainRepository::add also rejects duplicates, but we
        // check here for a friendlier error message with guidance.
        if self.domains.get(&pattern)?.is_some() {
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
        let cert_outcome = match self.certs.create_and_install(&pattern) {
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

        self.domains.add(registration.clone())?;

        Ok(RegisterResult {
            registration,
            cert_outcome,
        })
    }
}
