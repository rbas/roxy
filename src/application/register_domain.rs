use anyhow::{Result, bail};

use crate::domain::{DomainPattern, DomainRegistration, Route};

use super::ports::DomainRepository;

/// Result of a successful domain registration.
pub struct RegisterResult {
    pub registration: DomainRegistration,
}

/// Use case: register a new domain with routes.
pub struct RegisterDomain<'a> {
    domains: &'a dyn DomainRepository,
}

impl<'a> RegisterDomain<'a> {
    pub fn new(domains: &'a dyn DomainRepository) -> Self {
        Self { domains }
    }

    /// Validate inputs and persist the registration.
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

        let mut registration = DomainRegistration::new(pattern, routes);
        // HTTPS is provided by the daemon's CA-backed SNI resolver. Registering
        // a route never needs to create or persist a leaf certificate.
        registration.enable_https();

        self.domains.add(registration.clone())?;

        Ok(RegisterResult { registration })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    #[test]
    fn registers_domain_with_https() {
        let repo = InMemoryDomainRepository::new();
        let svc = RegisterDomain::new(&repo);

        let result = svc
            .execute(exact("myapp.roxy"), vec![proxy_route("/", 3000)])
            .unwrap();

        assert!(result.registration.is_https_enabled());
        assert!(repo.get(&exact("myapp.roxy")).unwrap().is_some());
    }

    #[test]
    fn rejects_empty_routes() {
        let repo = InMemoryDomainRepository::new();
        let svc = RegisterDomain::new(&repo);

        let err = svc.execute(exact("myapp.roxy"), vec![]).err().unwrap();
        assert!(err.to_string().contains("At least one route"));
    }

    #[test]
    fn rejects_duplicate_domain() {
        let repo = InMemoryDomainRepository::new();
        let svc = RegisterDomain::new(&repo);

        svc.execute(exact("myapp.roxy"), vec![proxy_route("/", 3000)])
            .unwrap();

        let err = svc
            .execute(exact("myapp.roxy"), vec![proxy_route("/", 4000)])
            .err()
            .unwrap();
        assert!(err.to_string().contains("already registered"));
    }

    #[test]
    fn multiple_routes_are_persisted() {
        let repo = InMemoryDomainRepository::new();
        let svc = RegisterDomain::new(&repo);

        let routes = vec![proxy_route("/", 3000), proxy_route("/api", 3001)];
        let result = svc.execute(exact("myapp.roxy"), routes).unwrap();

        assert_eq!(result.registration.routes().len(), 2);
    }
}
