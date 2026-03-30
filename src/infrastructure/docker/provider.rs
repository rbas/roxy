use std::sync::{Arc, RwLock};

use bollard::Docker;

use crate::application::ports::RegistrationProvider;
use crate::domain::DomainRegistration;

/// Supplies domain registrations discovered from running Docker containers.
///
/// The watcher task updates the internal state via `RwLock`; the `load()`
/// method reads it. Uses `std::sync::RwLock` (not tokio) since contention
/// is near-zero and the trait is sync.
pub struct DockerProvider {
    state: Arc<RwLock<Vec<DomainRegistration>>>,
    docker: Docker,
}

impl DockerProvider {
    pub fn new(docker: Docker) -> Self {
        Self {
            state: Arc::new(RwLock::new(Vec::new())),
            docker,
        }
    }

    /// Shared reference to the internal state, for use by the watcher task.
    pub fn state(&self) -> Arc<RwLock<Vec<DomainRegistration>>> {
        self.state.clone()
    }

    /// Reference to the Docker client, for use by the watcher task.
    pub fn docker(&self) -> &Docker {
        &self.docker
    }
}

impl RegistrationProvider for DockerProvider {
    fn name(&self) -> &str {
        "docker"
    }

    fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
        Ok(self
            .state
            .read()
            .map_err(|e| anyhow::anyhow!("Docker provider lock poisoned: {e}"))?
            .clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a Docker client that doesn't require a running Docker daemon.
    /// These tests only exercise in-memory state; no Docker API calls are made.
    fn test_docker() -> Docker {
        Docker::connect_with_http("http://localhost:1", 1, bollard::API_DEFAULT_VERSION).unwrap()
    }

    #[test]
    fn provider_name() {
        let provider = DockerProvider::new(test_docker());
        assert_eq!(provider.name(), "docker");
    }

    #[test]
    fn load_returns_empty_initially() {
        let provider = DockerProvider::new(test_docker());
        let result = provider.load().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_returns_state_after_update() {
        use crate::domain::{DomainName, DomainPattern, Route};

        let provider = DockerProvider::new(test_docker());

        // Simulate watcher updating state
        {
            let name = DomainName::new("web.myproject.roxy").unwrap();
            let pattern = DomainPattern::Exact(name);
            let routes = vec![Route::parse("/=3000").unwrap()];
            let reg = DomainRegistration::new(pattern, routes);

            let state_arc = provider.state();
            let mut state = state_arc.write().unwrap();
            state.push(reg);
        }

        let result = provider.load().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].domain().as_str(), "web.myproject.roxy");
    }
}
