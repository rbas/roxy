mod dto;
pub mod watcher;

// Re-export shared config types so existing CLI/daemon code keeps compiling.
pub use crate::config::{DaemonConfig, DockerConfig, RoxyPaths};

use crate::application::ports::{ConfigLoadError, ConfigLoader, DomainRepository, RepositoryError};
use crate::domain::value_objects::{RouteTarget, RouteTargetError};
use crate::domain::{DomainPattern, DomainRegistration};
use dto::RegistrationDto;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("Failed to serialize config: {0}")]
    SerializeError(#[from] toml::ser::Error),

    #[error("Domain already registered: {0}")]
    DomainExists(String),

    #[error("Domain not found: {0}")]
    DomainNotFound(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid domain '{0}': {1}")]
    InvalidDomain(String, String),
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub daemon: DaemonConfig,

    #[serde(default)]
    pub paths: RoxyPaths,

    #[serde(default)]
    pub docker: DockerConfig,

    #[serde(default)]
    domains: HashMap<String, RegistrationDto>,
}

impl Config {
    /// Convert all stored DTOs to domain registrations.
    pub fn registrations(&self) -> Vec<DomainRegistration> {
        self.domains
            .values()
            .cloned()
            .map(DomainRegistration::from)
            .collect()
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        self.daemon.validate().map_err(ConfigError::InvalidConfig)?;

        for (name, dto) in &self.domains {
            let registration = DomainRegistration::from(dto.clone());
            // Structural validation (invariants enforced at construction)
            registration
                .validate()
                .map_err(|e| ConfigError::InvalidDomain(name.clone(), e.to_string()))?;
            // Infrastructure-level validation: check filesystem paths exist
            for route in registration.routes() {
                validate_route_path(route.target()).map_err(|e: RouteTargetError| {
                    ConfigError::InvalidDomain(name.clone(), e.to_string())
                })?;
            }
        }

        Ok(())
    }
}

/// Validate that a static files target points to an existing directory.
///
/// This is an infrastructure concern (filesystem I/O), kept out of the domain layer.
fn validate_route_path(target: &RouteTarget) -> Result<(), RouteTargetError> {
    if let RouteTarget::StaticFiles(path) = target {
        if !path.exists() {
            return Err(RouteTargetError::PathNotFound(path.clone()));
        }
        if !path.is_dir() {
            return Err(RouteTargetError::NotADirectory(path.clone()));
        }
    }
    Ok(())
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    /// Create a new ConfigStore pointing at the given config file path
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn ensure_config_dir(&self) -> Result<(), ConfigError> {
        if let Some(dir) = self.path.parent()
            && !dir.exists()
        {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    pub fn load(&self) -> Result<Config, ConfigError> {
        if !self.path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&self.path)?;
        let config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self, config: &Config) -> Result<(), ConfigError> {
        self.ensure_config_dir()?;

        let content = toml::to_string_pretty(config)?;
        let temporary = self.path.with_extension("toml.tmp");
        fs::write(&temporary, content)?;
        fs::rename(temporary, &self.path)?;
        Ok(())
    }

    pub fn add_domain(&self, registration: DomainRegistration) -> Result<(), ConfigError> {
        let mut config = self.load()?;

        let key = registration.display_pattern();
        if config.domains.contains_key(&key) {
            return Err(ConfigError::DomainExists(key));
        }

        config.domains.insert(key, registration.into());
        self.save(&config)
    }

    pub fn remove_domain(
        &self,
        pattern: &DomainPattern,
    ) -> Result<DomainRegistration, ConfigError> {
        let mut config = self.load()?;

        let key = pattern.display_pattern();
        let dto = config
            .domains
            .remove(&key)
            .ok_or(ConfigError::DomainNotFound(key))?;

        self.save(&config)?;
        Ok(dto.into())
    }

    pub fn get_domain(
        &self,
        pattern: &DomainPattern,
    ) -> Result<Option<DomainRegistration>, ConfigError> {
        let config = self.load()?;
        let key = pattern.display_pattern();
        Ok(config
            .domains
            .get(&key)
            .cloned()
            .map(DomainRegistration::from))
    }

    pub fn update_domain(&self, registration: DomainRegistration) -> Result<(), ConfigError> {
        let mut config = self.load()?;

        let key = registration.display_pattern();
        if !config.domains.contains_key(&key) {
            return Err(ConfigError::DomainNotFound(key));
        }

        config.domains.insert(key, registration.into());
        self.save(&config)
    }

    /// Check whether the config file exists on disk.
    pub fn config_exists(&self) -> bool {
        self.path.exists()
    }

    pub fn list_domains(&self) -> Result<Vec<DomainRegistration>, ConfigError> {
        let config = self.load()?;
        Ok(config
            .domains
            .into_values()
            .map(DomainRegistration::from)
            .collect())
    }
}

fn map_config_error(e: ConfigError) -> ConfigLoadError {
    match e {
        ConfigError::InvalidConfig(msg) => ConfigLoadError::Invalid(msg),
        other => ConfigLoadError::IoFailed(other.into()),
    }
}

impl ConfigLoader for ConfigStore {
    fn load(&self) -> Result<(DaemonConfig, RoxyPaths), ConfigLoadError> {
        let config = ConfigStore::load(self).map_err(map_config_error)?;
        Ok((config.daemon, config.paths))
    }

    fn save_defaults(&self) -> Result<(), ConfigLoadError> {
        let config = Config::default();
        self.save(&config).map_err(map_config_error)
    }

    fn exists(&self) -> bool {
        self.config_exists()
    }
}

impl DomainRepository for ConfigStore {
    fn get(&self, pattern: &DomainPattern) -> Result<Option<DomainRegistration>, RepositoryError> {
        self.get_domain(pattern)
            .map_err(|e| RepositoryError::StorageFailed(e.into()))
    }

    fn list(&self) -> Result<Vec<DomainRegistration>, RepositoryError> {
        self.list_domains()
            .map_err(|e| RepositoryError::StorageFailed(e.into()))
    }

    fn add(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        self.add_domain(registration).map_err(|e| match e {
            ConfigError::DomainExists(d) => RepositoryError::DomainExists(d),
            other => RepositoryError::StorageFailed(other.into()),
        })
    }

    fn update(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        self.update_domain(registration).map_err(|e| match e {
            ConfigError::DomainNotFound(d) => RepositoryError::DomainNotFound(d),
            other => RepositoryError::StorageFailed(other.into()),
        })
    }

    fn remove(&self, pattern: &DomainPattern) -> Result<(), RepositoryError> {
        self.remove_domain(pattern).map_err(|e| match e {
            ConfigError::DomainNotFound(d) => RepositoryError::DomainNotFound(d),
            other => RepositoryError::StorageFailed(other.into()),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- DaemonConfig::validate ---

    #[test]
    fn default_config_is_valid() {
        let config = DaemonConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn custom_valid_config() {
        let config = DaemonConfig {
            http_port: 8080,
            https_port: 8443,
            dns_port: 5353,
            log_level: "debug".to_string(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn zero_http_port_is_invalid() {
        let config = DaemonConfig {
            http_port: 0,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("http_port cannot be 0"));
    }

    #[test]
    fn zero_https_port_is_invalid() {
        let config = DaemonConfig {
            https_port: 0,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("https_port cannot be 0"));
    }

    #[test]
    fn zero_dns_port_is_invalid() {
        let config = DaemonConfig {
            dns_port: 0,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("dns_port cannot be 0"));
    }

    #[test]
    fn duplicate_http_and_https_ports_is_invalid() {
        let config = DaemonConfig {
            http_port: 8080,
            https_port: 8080,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("must be different"));
    }

    #[test]
    fn duplicate_http_and_dns_ports_is_invalid() {
        let config = DaemonConfig {
            http_port: 1053,
            https_port: 443,
            dns_port: 1053,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("must all be different"));
    }

    #[test]
    fn duplicate_https_and_dns_ports_is_invalid() {
        let config = DaemonConfig {
            http_port: 80,
            https_port: 1053,
            dns_port: 1053,
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("must all be different"));
    }

    #[test]
    fn invalid_log_level_is_rejected() {
        let config = DaemonConfig {
            log_level: "verbose".to_string(),
            ..DaemonConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("Invalid log_level"));
    }

    #[test]
    fn valid_log_levels_are_accepted() {
        for level in ["error", "warn", "info", "debug"] {
            let config = DaemonConfig {
                log_level: level.to_string(),
                ..DaemonConfig::default()
            };
            assert!(
                config.validate().is_ok(),
                "level '{}' should be valid",
                level
            );
        }
    }

    // --- Config::validate ---

    #[test]
    fn config_validate_delegates_to_daemon() {
        let config = Config {
            daemon: DaemonConfig {
                http_port: 0,
                ..DaemonConfig::default()
            },
            ..Config::default()
        };
        assert!(config.validate().is_err());
    }
}
