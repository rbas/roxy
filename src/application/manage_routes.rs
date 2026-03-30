use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, DomainRegistration, PathPrefix, Route, RouteTarget};

use super::ports::DomainRepository;

/// Use case: manage routes for an existing domain registration.
pub struct ManageRoutes<'a> {
    domains: &'a dyn DomainRepository,
}

impl<'a> ManageRoutes<'a> {
    pub fn new(domains: &'a dyn DomainRepository) -> Self {
        Self { domains }
    }

    /// Add a route to an existing domain. Returns the added route.
    pub fn add_route(
        &self,
        pattern: &DomainPattern,
        path_prefix: PathPrefix,
        route_target: RouteTarget,
    ) -> Result<Route> {
        let mut registration = self
            .domains
            .get(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' not registered", pattern))?;

        let route = Route::new(path_prefix, route_target);
        registration.add_route(route.clone())?;
        self.domains.update(registration)?;

        Ok(route)
    }

    /// List routes for an existing domain. Returns the full registration.
    pub fn list_routes(&self, pattern: &DomainPattern) -> Result<DomainRegistration> {
        self.domains
            .get(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' not registered", pattern))
    }

    /// Remove a route from an existing domain.
    pub fn remove_route(&self, pattern: &DomainPattern, path_prefix: &PathPrefix) -> Result<()> {
        let mut registration = self
            .domains
            .get(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' not registered", pattern))?;

        registration.remove_route(path_prefix)?;
        self.domains.update(registration)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;
    use crate::domain::ProxyTarget;

    #[test]
    fn adds_route_to_existing_domain() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = ManageRoutes::new(&repo);

        let route = svc
            .add_route(
                &exact("myapp.roxy"),
                PathPrefix::new("/api").unwrap(),
                RouteTarget::Proxy(ProxyTarget::parse("3001").unwrap()),
            )
            .unwrap();

        assert_eq!(route.path().as_str(), "/api");
        // Verify persisted
        let reg = repo.get(&exact("myapp.roxy")).unwrap().unwrap();
        assert_eq!(reg.routes().len(), 2);
    }

    #[test]
    fn add_route_fails_for_unknown_domain() {
        let repo = InMemoryDomainRepository::new();
        let svc = ManageRoutes::new(&repo);

        let err = svc
            .add_route(
                &exact("unknown.roxy"),
                PathPrefix::new("/api").unwrap(),
                RouteTarget::Proxy(ProxyTarget::parse("3001").unwrap()),
            )
            .err()
            .unwrap();
        assert!(err.to_string().contains("not registered"));
    }

    #[test]
    fn add_route_fails_for_duplicate_path() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = ManageRoutes::new(&repo);

        let err = svc
            .add_route(
                &exact("myapp.roxy"),
                PathPrefix::new("/").unwrap(),
                RouteTarget::Proxy(ProxyTarget::parse("4000").unwrap()),
            )
            .err()
            .unwrap();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn removes_route_from_domain() {
        let repo = InMemoryDomainRepository::with_domains(vec![DomainRegistration::new(
            exact("myapp.roxy"),
            vec![proxy_route("/", 3000), proxy_route("/api", 3001)],
        )]);
        let svc = ManageRoutes::new(&repo);

        svc.remove_route(&exact("myapp.roxy"), &PathPrefix::new("/api").unwrap())
            .unwrap();

        let reg = repo.get(&exact("myapp.roxy")).unwrap().unwrap();
        assert_eq!(reg.routes().len(), 1);
    }

    #[test]
    fn remove_last_route_fails() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = ManageRoutes::new(&repo);

        let err = svc
            .remove_route(&exact("myapp.roxy"), &PathPrefix::new("/").unwrap())
            .err()
            .unwrap();
        assert!(err.to_string().contains("Cannot remove"));
    }

    #[test]
    fn list_routes_returns_registration() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = ManageRoutes::new(&repo);

        let reg = svc.list_routes(&exact("myapp.roxy")).unwrap();
        assert_eq!(reg.routes().len(), 1);
    }

    #[test]
    fn list_routes_fails_for_unknown_domain() {
        let repo = InMemoryDomainRepository::new();
        let svc = ManageRoutes::new(&repo);

        let err = svc.list_routes(&exact("unknown.roxy")).err().unwrap();
        assert!(err.to_string().contains("not registered"));
    }
}
