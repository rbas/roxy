use crate::domain::{DomainPattern, DomainRegistration};

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Domain already registered: {0}")]
    DomainExists(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("{0}")]
    StorageFailed(#[source] anyhow::Error),
}

/// Port for persisting and querying domain registrations.
pub trait DomainRepository {
    /// Look up a single registration by pattern.
    fn get(&self, pattern: &DomainPattern) -> Result<Option<DomainRegistration>, RepositoryError>;

    /// Return all registered domains.
    fn list(&self) -> Result<Vec<DomainRegistration>, RepositoryError>;

    /// Persist a new registration. Fails if the domain already exists.
    fn add(&self, registration: DomainRegistration) -> Result<(), RepositoryError>;

    /// Update an existing registration. Fails if not found.
    fn update(&self, registration: DomainRegistration) -> Result<(), RepositoryError>;

    /// Remove a registration by pattern. Fails if not found.
    fn remove(&self, pattern: &DomainPattern) -> Result<(), RepositoryError>;
}
