use std::fmt;

use super::{DomainName, DomainPattern, PathPrefix, Route};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("Route for path '{0}' already exists")]
    RouteExists(String),

    #[error("No route found for path '{0}'")]
    RouteNotFound(String),

    #[error("Cannot remove the last route - unregister the domain instead")]
    CannotRemoveLastRoute,
}

/// Where a domain registration originated from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationSource {
    /// Loaded from the config file.
    Config,
    /// Discovered dynamically from an external provider (Docker, etc.).
    External,
}

impl fmt::Display for RegistrationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config => write!(f, "config"),
            Self::External => write!(f, "external"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DomainRegistration {
    pattern: DomainPattern,
    routes: Vec<Route>,
    https_enabled: bool,
    source: RegistrationSource,
}

impl DomainRegistration {
    pub fn new(pattern: DomainPattern, routes: Vec<Route>) -> Self {
        Self {
            pattern,
            routes,
            https_enabled: true,
            source: RegistrationSource::Config,
        }
    }

    /// Create a registration with an explicit source.
    pub fn with_source(
        pattern: DomainPattern,
        routes: Vec<Route>,
        source: RegistrationSource,
    ) -> Self {
        Self {
            pattern,
            routes,
            https_enabled: true,
            source,
        }
    }

    // --- Accessors ---

    pub fn pattern(&self) -> &DomainPattern {
        &self.pattern
    }

    pub fn domain(&self) -> &DomainName {
        self.pattern.base_domain()
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn is_https_enabled(&self) -> bool {
        self.https_enabled
    }

    pub fn source(&self) -> RegistrationSource {
        self.source
    }

    pub fn is_wildcard(&self) -> bool {
        self.pattern.is_wildcard()
    }

    // --- Delegated pattern methods ---

    pub fn display_pattern(&self) -> String {
        self.pattern.display_pattern()
    }

    // --- Mutators ---

    pub fn enable_https(&mut self) {
        self.https_enabled = true;
    }

    /// Find the best matching route for a request path.
    /// Returns None if no route matches.
    /// Uses longest prefix matching (most specific match wins).
    pub fn match_route(&self, request_path: &str) -> Option<&Route> {
        self.routes
            .iter()
            .filter(|r| r.path().matches(request_path))
            .max_by_key(|r| r.path().len())
    }

    /// Add a route to this registration.
    /// Returns error if a route with the same path already exists.
    pub fn add_route(&mut self, route: Route) -> Result<(), RegistrationError> {
        if self.routes.iter().any(|r| r.path() == route.path()) {
            return Err(RegistrationError::RouteExists(route.path().to_string()));
        }
        self.routes.push(route);
        Ok(())
    }

    /// Remove a route by its path prefix.
    /// Returns error if no route with that path exists or if it's the last route.
    pub fn remove_route(&mut self, path: &PathPrefix) -> Result<(), RegistrationError> {
        if self.routes.len() == 1 {
            return Err(RegistrationError::CannotRemoveLastRoute);
        }

        let len_before = self.routes.len();
        self.routes.retain(|r| r.path() != path);

        if self.routes.len() == len_before {
            return Err(RegistrationError::RouteNotFound(path.to_string()));
        }
        Ok(())
    }

    /// Validate structural invariants of this registration.
    ///
    /// Filesystem checks (path exists, is directory) are NOT done here —
    /// those belong in the infrastructure or application layer.
    pub fn validate(&self) -> Result<(), RegistrationError> {
        // Structural invariants only — no I/O
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProxyTarget;

    fn make_pattern(name: &str) -> DomainPattern {
        DomainPattern::Exact(DomainName::new(name).unwrap())
    }

    fn proxy_route(path: &str, port: u16) -> Route {
        Route::new(
            PathPrefix::new(path).unwrap(),
            crate::domain::RouteTarget::Proxy(ProxyTarget::parse(&port.to_string()).unwrap()),
        )
    }

    // --- Constructor ---

    #[test]
    fn new_creates_https_registration() {
        let reg = DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        assert!(reg.is_https_enabled());
        assert_eq!(reg.routes().len(), 1);
        assert_eq!(reg.domain().as_str(), "myapp.roxy");
    }

    #[test]
    fn with_source_sets_source() {
        let reg = DomainRegistration::with_source(
            make_pattern("myapp.roxy"),
            vec![proxy_route("/", 3000)],
            RegistrationSource::External,
        );
        assert_eq!(reg.source(), RegistrationSource::External);
    }

    // --- enable_https ---

    #[test]
    fn enable_https_sets_flag() {
        let mut reg =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        assert!(reg.is_https_enabled());
        reg.enable_https();
        assert!(reg.is_https_enabled());
    }

    // --- match_route: longest prefix wins ---

    #[test]
    fn match_route_returns_exact_match() {
        let reg = DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        let matched = reg.match_route("/").unwrap();
        assert_eq!(matched.path().as_str(), "/");
    }

    #[test]
    fn match_route_longest_prefix_wins() {
        let reg = DomainRegistration::new(
            make_pattern("myapp.roxy"),
            vec![proxy_route("/", 3000), proxy_route("/api", 4000)],
        );

        // /api/users should match /api (more specific) not /
        let matched = reg.match_route("/api/users").unwrap();
        assert_eq!(matched.path().as_str(), "/api");

        // / should match root
        let matched = reg.match_route("/").unwrap();
        assert_eq!(matched.path().as_str(), "/");
    }

    #[test]
    fn match_route_returns_none_when_no_match() {
        let reg =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/api", 4000)]);
        // /other doesn't match /api prefix
        assert!(reg.match_route("/other").is_none());
    }

    // --- add_route ---

    #[test]
    fn add_route_succeeds_for_new_path() {
        let mut reg =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        assert!(reg.add_route(proxy_route("/api", 4000)).is_ok());
        assert_eq!(reg.routes().len(), 2);
    }

    #[test]
    fn add_route_fails_for_duplicate_path() {
        let mut reg =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        let result = reg.add_route(proxy_route("/", 4000));
        assert!(matches!(result, Err(RegistrationError::RouteExists(_))));
    }

    // --- remove_route ---

    #[test]
    fn remove_route_succeeds() {
        let mut reg = DomainRegistration::new(
            make_pattern("myapp.roxy"),
            vec![proxy_route("/", 3000), proxy_route("/api", 4000)],
        );
        let path = PathPrefix::new("/api").unwrap();
        assert!(reg.remove_route(&path).is_ok());
        assert_eq!(reg.routes().len(), 1);
    }

    #[test]
    fn remove_route_fails_for_last_route() {
        let mut reg =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        let path = PathPrefix::new("/").unwrap();
        let result = reg.remove_route(&path);
        assert!(matches!(
            result,
            Err(RegistrationError::CannotRemoveLastRoute)
        ));
    }

    #[test]
    fn remove_route_fails_for_nonexistent_path() {
        let mut reg = DomainRegistration::new(
            make_pattern("myapp.roxy"),
            vec![proxy_route("/", 3000), proxy_route("/api", 4000)],
        );
        let path = PathPrefix::new("/other").unwrap();
        let result = reg.remove_route(&path);
        assert!(matches!(result, Err(RegistrationError::RouteNotFound(_))));
    }

    // --- validate ---

    #[test]
    fn validate_passes_for_proxy_routes() {
        let reg = DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        assert!(reg.validate().is_ok());
    }

    // --- display_pattern ---

    #[test]
    fn display_pattern_delegates_to_domain_pattern() {
        let exact =
            DomainRegistration::new(make_pattern("myapp.roxy"), vec![proxy_route("/", 3000)]);
        assert_eq!(exact.display_pattern(), "myapp.roxy");

        let wildcard = DomainRegistration::new(
            DomainPattern::Wildcard(DomainName::new("myapp.roxy").unwrap()),
            vec![proxy_route("/", 3000)],
        );
        assert_eq!(wildcard.display_pattern(), "*.myapp.roxy");
    }
}
