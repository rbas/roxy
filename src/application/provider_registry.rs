use std::sync::Arc;

use tracing::warn;

use super::ports::RegistrationProvider;
use crate::domain::DomainRegistration;

/// Aggregates registrations from multiple providers.
///
/// When `load()` is called, each provider is queried and results are
/// concatenated. Individual provider errors are logged but don't fail
/// the merge — other providers still contribute their registrations.
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn RegistrationProvider>>,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn add(&mut self, provider: Arc<dyn RegistrationProvider>) {
        self.providers.push(provider);
    }
}

impl RegistrationProvider for ProviderRegistry {
    fn name(&self) -> &str {
        "aggregator"
    }

    fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
        let mut all = Vec::new();

        for provider in &self.providers {
            match provider.load() {
                Ok(regs) => all.extend(regs),
                Err(e) => {
                    warn!(
                        provider = provider.name(),
                        error = %e,
                        "Provider failed to load registrations, skipping"
                    );
                }
            }
        }

        Ok(all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedProvider {
        name: &'static str,
        registrations: Vec<DomainRegistration>,
    }

    impl RegistrationProvider for FixedProvider {
        fn name(&self) -> &str {
            self.name
        }
        fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
            Ok(self.registrations.clone())
        }
    }

    struct FailingProvider;

    impl RegistrationProvider for FailingProvider {
        fn name(&self) -> &str {
            "failing"
        }
        fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
            Err(anyhow::anyhow!("provider error"))
        }
    }

    fn test_reg(domain: &str) -> DomainRegistration {
        use crate::domain::{DomainName, DomainPattern, Route};
        let name = DomainName::new(domain).unwrap();
        let pattern = DomainPattern::Exact(name);
        let routes = vec![Route::parse("/=3000").unwrap()];
        DomainRegistration::new(pattern, routes)
    }

    #[test]
    fn empty_registry_returns_empty() {
        let registry = ProviderRegistry::new();
        let result = registry.load().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn single_provider() {
        let mut registry = ProviderRegistry::new();
        registry.add(Arc::new(FixedProvider {
            name: "test",
            registrations: vec![test_reg("app.roxy")],
        }));

        let result = registry.load().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].domain().as_str(), "app.roxy");
    }

    #[test]
    fn merges_multiple_providers() {
        let mut registry = ProviderRegistry::new();
        registry.add(Arc::new(FixedProvider {
            name: "config",
            registrations: vec![test_reg("app.roxy")],
        }));
        registry.add(Arc::new(FixedProvider {
            name: "docker",
            registrations: vec![test_reg("web.myproject.roxy")],
        }));

        let result = registry.load().unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn failing_provider_does_not_block_others() {
        let mut registry = ProviderRegistry::new();
        registry.add(Arc::new(FixedProvider {
            name: "config",
            registrations: vec![test_reg("app.roxy")],
        }));
        registry.add(Arc::new(FailingProvider));

        let result = registry.load().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].domain().as_str(), "app.roxy");
    }

    #[test]
    fn all_providers_failing_returns_empty() {
        let mut registry = ProviderRegistry::new();
        registry.add(Arc::new(FailingProvider));

        let result = registry.load().unwrap();
        assert!(result.is_empty());
    }
}
