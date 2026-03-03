use crate::domain::DomainRegistration;

/// Port for components that supply domain registrations to the daemon.
///
/// Each provider is a source of registrations (config file, Docker, etc.).
/// The daemon merges registrations from all providers.
pub trait RegistrationProvider: Send + Sync {
    /// Human-readable name ("config-file", "docker").
    #[allow(dead_code)] // Used by tests and future multi-provider logging.
    fn name(&self) -> &str;

    /// Load current registrations from this source.
    fn load(&self) -> anyhow::Result<Vec<DomainRegistration>>;
}
