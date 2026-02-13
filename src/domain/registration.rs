use super::{DomainName, PathPrefix, Route, RouteTarget};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrationError {
    #[error("Target path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Target path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Route for path '{0}' already exists")]
    RouteExists(String),

    #[error("No route found for path '{0}'")]
    RouteNotFound(String),

    #[error("Cannot remove the last route - unregister the domain instead")]
    CannotRemoveLastRoute,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRegistration {
    pub domain: DomainName,
    pub routes: Vec<Route>,
    pub https_enabled: bool,
    #[serde(default)]
    pub wildcard: bool,
}

impl DomainRegistration {
    pub fn new(domain: DomainName, routes: Vec<Route>) -> Self {
        Self {
            domain,
            routes,
            https_enabled: false, // Will be enabled after cert generation
            wildcard: false,
        }
    }

    pub fn new_wildcard(domain: DomainName, routes: Vec<Route>) -> Self {
        Self {
            domain,
            routes,
            https_enabled: false, // Will be enabled after cert generation
            wildcard: true,
        }
    }

    pub fn enable_https(&mut self) {
        self.https_enabled = true;
    }

    pub fn config_key(&self) -> String {
        if self.wildcard {
            format!("*.{}", self.domain.as_str())
        } else {
            self.domain.as_str().to_string()
        }
    }

    pub fn display_pattern(&self) -> String {
        self.config_key()
    }

    /// Find the best matching route for a request path.
    /// Returns None if no route matches.
    /// Uses longest prefix matching (most specific match wins).
    pub fn match_route(&self, request_path: &str) -> Option<&Route> {
        self.routes
            .iter()
            .filter(|r| r.path.matches(request_path))
            .max_by_key(|r| r.path.len())
    }

    /// Add a route to this registration.
    /// Returns error if a route with the same path already exists.
    pub fn add_route(&mut self, route: Route) -> Result<(), RegistrationError> {
        if self.routes.iter().any(|r| r.path == route.path) {
            return Err(RegistrationError::RouteExists(route.path.to_string()));
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
        self.routes.retain(|r| &r.path != path);

        if self.routes.len() == len_before {
            return Err(RegistrationError::RouteNotFound(path.to_string()));
        }
        Ok(())
    }

    /// Validate that the registration is still valid (e.g., paths exist)
    pub fn validate(&self) -> Result<(), RegistrationError> {
        for route in &self.routes {
            if let RouteTarget::StaticFiles(path) = &route.target {
                if !path.exists() {
                    return Err(RegistrationError::PathNotFound(path.clone()));
                }
                if !path.is_dir() {
                    return Err(RegistrationError::NotADirectory(path.clone()));
                }
            }
            // Proxy targets don't need validation - the service may not be running yet
        }
        Ok(())
    }
}
