use crate::application::ports::{DomainRepository, RepositoryError};
use crate::domain::{DomainPattern, DomainRegistration};
use crate::infrastructure::config::{ConfigError, ConfigStore};

/// Adapter that bridges [`ConfigStore`] domain operations to the
/// [`DomainRepository`] port.
pub struct DomainRepositoryAdapter<'a> {
    inner: &'a ConfigStore,
}

impl<'a> DomainRepositoryAdapter<'a> {
    pub fn new(inner: &'a ConfigStore) -> Self {
        Self { inner }
    }
}

impl DomainRepository for DomainRepositoryAdapter<'_> {
    fn get(&self, pattern: &DomainPattern) -> Result<Option<DomainRegistration>, RepositoryError> {
        self.inner
            .get_domain(pattern)
            .map_err(|e| RepositoryError::StorageFailed(e.into()))
    }

    fn list(&self) -> Result<Vec<DomainRegistration>, RepositoryError> {
        self.inner
            .list_domains()
            .map_err(|e| RepositoryError::StorageFailed(e.into()))
    }

    fn add(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        self.inner.add_domain(registration).map_err(|e| match e {
            ConfigError::DomainExists(d) => RepositoryError::DomainExists(d),
            other => RepositoryError::StorageFailed(other.into()),
        })
    }

    fn update(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        self.inner.update_domain(registration).map_err(|e| match e {
            ConfigError::DomainNotFound(d) => RepositoryError::DomainNotFound(d),
            other => RepositoryError::StorageFailed(other.into()),
        })
    }

    fn remove(&self, pattern: &DomainPattern) -> Result<(), RepositoryError> {
        self.inner.remove_domain(pattern).map_err(|e| match e {
            ConfigError::DomainNotFound(d) => RepositoryError::DomainNotFound(d),
            other => RepositoryError::StorageFailed(other.into()),
        })?;
        Ok(())
    }
}
