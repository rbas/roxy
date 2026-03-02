use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, PathPrefix, Route, RouteTarget};

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
