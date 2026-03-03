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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;
    use crate::domain::{PathPrefix, ProxyTarget, Route, RouteTarget};

    fn proxy_route(path: &str, port: u16) -> Route {
        Route::new(
            PathPrefix::new(path).unwrap(),
            RouteTarget::Proxy(ProxyTarget::parse(&port.to_string()).unwrap()),
        )
    }

    fn exact(name: &str) -> DomainPattern {
        DomainPattern::from_name(name, false).unwrap()
    }

    fn registration(name: &str) -> DomainRegistration {
        DomainRegistration::new(exact(name), vec![proxy_route("/", 3000)])
    }

    #[test]
    fn unregisters_domain_and_removes_cert() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let certs = InMemoryCertificateManager::new();
        // Install cert first so it exists
        certs.create_and_install(&exact("myapp.roxy")).unwrap();
        let svc = UnregisterDomain::new(&repo, &certs);

        let result = svc.execute(&exact("myapp.roxy")).unwrap();

        assert!(matches!(result.cert_outcome, StepOutcome::Success(_)));
        assert!(repo.get(&exact("myapp.roxy")).unwrap().is_none());
        assert!(!certs.exists(&exact("myapp.roxy")));
    }

    #[test]
    fn unregisters_domain_without_cert() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let certs = InMemoryCertificateManager::new();
        let svc = UnregisterDomain::new(&repo, &certs);

        let result = svc.execute(&exact("myapp.roxy")).unwrap();

        assert!(matches!(result.cert_outcome, StepOutcome::Skipped(_)));
        assert!(repo.get(&exact("myapp.roxy")).unwrap().is_none());
    }

    #[test]
    fn fails_for_unknown_domain() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = UnregisterDomain::new(&repo, &certs);

        let err = svc.execute(&exact("unknown.roxy")).err().unwrap();
        assert!(err.to_string().contains("not registered"));
    }

    #[test]
    fn preview_returns_registration() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let certs = InMemoryCertificateManager::new();
        let svc = UnregisterDomain::new(&repo, &certs);

        let reg = svc.preview(&exact("myapp.roxy")).unwrap();
        assert_eq!(reg.domain().as_str(), "myapp.roxy");
    }
}
