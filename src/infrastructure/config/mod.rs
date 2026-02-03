use crate::domain::{DomainName, DomainRegistration};
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
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub domains: HashMap<String, DomainRegistration>,
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Self {
        let config_dir = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".roxy");

        Self {
            path: config_dir.join("config.toml"),
        }
    }

    pub fn config_dir(&self) -> PathBuf {
        self.path.parent().unwrap().to_path_buf()
    }

    fn ensure_config_dir(&self) -> Result<(), ConfigError> {
        let dir = self.config_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
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
        fs::write(&self.path, content)?;
        Ok(())
    }

    pub fn add_domain(&self, registration: DomainRegistration) -> Result<(), ConfigError> {
        let mut config = self.load()?;

        let key = registration.domain.as_str().to_string();
        if config.domains.contains_key(&key) {
            return Err(ConfigError::DomainExists(key));
        }

        config.domains.insert(key, registration);
        self.save(&config)
    }

    pub fn remove_domain(&self, domain: &DomainName) -> Result<DomainRegistration, ConfigError> {
        let mut config = self.load()?;

        let key = domain.as_str();
        let registration = config
            .domains
            .remove(key)
            .ok_or_else(|| ConfigError::DomainNotFound(key.to_string()))?;

        self.save(&config)?;
        Ok(registration)
    }

    pub fn get_domain(
        &self,
        domain: &DomainName,
    ) -> Result<Option<DomainRegistration>, ConfigError> {
        let config = self.load()?;
        Ok(config.domains.get(domain.as_str()).cloned())
    }

    pub fn list_domains(&self) -> Result<Vec<DomainRegistration>, ConfigError> {
        let config = self.load()?;
        Ok(config.domains.into_values().collect())
    }
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new()
    }
}
