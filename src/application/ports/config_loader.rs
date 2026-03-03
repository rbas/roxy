use crate::config::{DaemonConfig, RoxyPaths};

#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("{0}")]
    IoFailed(#[source] anyhow::Error),
}

/// Port for loading and persisting daemon configuration.
///
/// Only exposes the configuration values the application layer needs
/// (daemon settings and paths). Domain persistence is handled by
/// [`DomainRepository`](super::DomainRepository).
pub trait ConfigLoader {
    /// Load daemon configuration and paths from storage.
    /// Returns defaults if no config file exists.
    fn load(&self) -> Result<(DaemonConfig, RoxyPaths), ConfigLoadError>;

    /// Create a config file with default values.
    fn save_defaults(&self) -> Result<(), ConfigLoadError>;

    /// Check whether the configuration file exists on disk.
    fn exists(&self) -> bool;
}
