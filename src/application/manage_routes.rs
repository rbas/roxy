use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, PathPrefix, Route, RouteTarget};
use crate::infrastructure::config::ConfigStore;

/// Use case: manage routes for an existing domain registration.
pub struct ManageRoutes<'a> {
    config_store: &'a ConfigStore,
}

impl<'a> ManageRoutes<'a> {
    pub fn new(config_store: &'a ConfigStore) -> Self {
        Self { config_store }
    }

    /// Add a route to an existing domain. Returns the added route.
    pub fn add_route(
        &self,
        pattern: &DomainPattern,
        path_prefix: PathPrefix,
        route_target: RouteTarget,
    ) -> Result<Route> {
        let mut registration = self
            .config_store
            .get_domain(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' not registered", pattern))?;

        let route = Route::new(path_prefix, route_target);
        registration.add_route(route.clone())?;
        self.config_store.update_domain(registration)?;

        Ok(route)
    }

    /// Remove a route from an existing domain.
    pub fn remove_route(&self, pattern: &DomainPattern, path_prefix: &PathPrefix) -> Result<()> {
        let mut registration = self
            .config_store
            .get_domain(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' not registered", pattern))?;

        registration.remove_route(path_prefix)?;
        self.config_store.update_domain(registration)?;

        Ok(())
    }
}
