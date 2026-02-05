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

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("Invalid domain '{0}': {1}")]
    InvalidDomain(String, String),
}

fn default_http_port() -> u16 {
    80
}

fn default_https_port() -> u16 {
    443
}

fn default_dns_port() -> u16 {
    1053
}

fn default_log_level() -> String {
    "info".to_string()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_http_port")]
    pub http_port: u16,

    #[serde(default = "default_https_port")]
    pub https_port: u16,

    #[serde(default = "default_dns_port")]
    pub dns_port: u16,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            http_port: default_http_port(),
            https_port: default_https_port(),
            dns_port: default_dns_port(),
            log_level: default_log_level(),
        }
    }
}

impl DaemonConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.http_port == 0 {
            return Err(ConfigError::InvalidConfig("http_port cannot be 0".into()));
        }
        if self.https_port == 0 {
            return Err(ConfigError::InvalidConfig("https_port cannot be 0".into()));
        }
        if self.dns_port == 0 {
            return Err(ConfigError::InvalidConfig("dns_port cannot be 0".into()));
        }
        if self.http_port == self.https_port {
            return Err(ConfigError::InvalidConfig(
                "http_port and https_port must be different".into(),
            ));
        }

        let ports = [self.http_port, self.https_port, self.dns_port];
        let unique_ports: std::collections::HashSet<_> = ports.iter().collect();
        if unique_ports.len() != ports.len() {
            return Err(ConfigError::InvalidConfig(
                "http_port, https_port, and dns_port must all be different".into(),
            ));
        }

        let valid_levels = ["error", "warn", "info", "debug"];
        if !valid_levels.contains(&self.log_level.as_str()) {
            return Err(ConfigError::InvalidConfig(format!(
                "Invalid log_level '{}'. Must be one of: {}",
                self.log_level,
                valid_levels.join(", ")
            )));
        }

        Ok(())
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub daemon: DaemonConfig,

    #[serde(default)]
    pub domains: HashMap<String, DomainRegistration>,
}

impl Config {
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.daemon.validate()?;

        for (name, registration) in &self.domains {
            registration
                .validate()
                .map_err(|e| ConfigError::InvalidDomain(name.clone(), e.to_string()))?;
        }

        Ok(())
    }
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

    pub fn update_domain(&self, registration: DomainRegistration) -> Result<(), ConfigError> {
        let mut config = self.load()?;

        let key = registration.domain.as_str().to_string();
        if !config.domains.contains_key(&key) {
            return Err(ConfigError::DomainNotFound(key));
        }

        config.domains.insert(key, registration);
        self.save(&config)
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
