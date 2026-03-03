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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;
    use crate::domain::{PathPrefix, ProxyTarget, RouteTarget};

    fn proxy_route(path: &str, port: u16) -> Route {
        Route::new(
            PathPrefix::new(path).unwrap(),
            RouteTarget::Proxy(ProxyTarget::parse(&port.to_string()).unwrap()),
        )
    }

    fn pattern(name: &str) -> DomainPattern {
        DomainPattern::from_name(name, false).unwrap()
    }

    #[test]
    fn registers_domain_with_https() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = RegisterDomain::new(&repo, &certs);

        let result = svc
            .execute(pattern("myapp.roxy"), vec![proxy_route("/", 3000)])
            .unwrap();

        assert!(result.registration.is_https_enabled());
        assert!(matches!(result.cert_outcome, StepOutcome::Success(_)));
        assert!(repo.get(&pattern("myapp.roxy")).unwrap().is_some());
    }

    #[test]
    fn registers_domain_without_https_when_cert_fails() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::always_failing();
        let svc = RegisterDomain::new(&repo, &certs);

        let result = svc
            .execute(pattern("myapp.roxy"), vec![proxy_route("/", 3000)])
            .unwrap();

        assert!(!result.registration.is_https_enabled());
        assert!(matches!(result.cert_outcome, StepOutcome::Warning(_)));
        // Domain is still registered despite cert failure
        assert!(repo.get(&pattern("myapp.roxy")).unwrap().is_some());
    }

    #[test]
    fn rejects_empty_routes() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = RegisterDomain::new(&repo, &certs);

        let err = svc.execute(pattern("myapp.roxy"), vec![]).err().unwrap();
        assert!(err.to_string().contains("At least one route"));
    }

    #[test]
    fn rejects_duplicate_domain() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = RegisterDomain::new(&repo, &certs);

        svc.execute(pattern("myapp.roxy"), vec![proxy_route("/", 3000)])
            .unwrap();

        let err = svc
            .execute(pattern("myapp.roxy"), vec![proxy_route("/", 4000)])
            .err()
            .unwrap();
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn multiple_routes_are_persisted() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = RegisterDomain::new(&repo, &certs);

        let routes = vec![proxy_route("/", 3000), proxy_route("/api", 3001)];
        let result = svc.execute(pattern("myapp.roxy"), routes).unwrap();

        assert_eq!(result.registration.routes().len(), 2);
    }
}
