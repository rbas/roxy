use crate::infrastructure::config::Config;

#[derive(Debug, thiserror::Error)]
pub enum ConfigLoadError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("{0}")]
    IoFailed(#[source] anyhow::Error),
}

/// Port for loading and saving the daemon configuration file.
pub trait ConfigLoader {
    /// Load configuration from storage. Returns defaults if not found.
    fn load(&self) -> Result<Config, ConfigLoadError>;

    /// Persist configuration to storage.
    fn save(&self, config: &Config) -> Result<(), ConfigLoadError>;

    /// Check whether the configuration file exists on disk.
    fn exists(&self) -> bool;
}
