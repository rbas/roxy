use crate::application::ports::{ConfigLoadError, ConfigLoader};
use crate::infrastructure::config::{Config, ConfigError, ConfigStore};

/// Adapter that bridges [`ConfigStore`] config operations to the
/// [`ConfigLoader`] port.
pub struct ConfigLoaderAdapter<'a> {
    inner: &'a ConfigStore,
}

impl<'a> ConfigLoaderAdapter<'a> {
    pub fn new(inner: &'a ConfigStore) -> Self {
        Self { inner }
    }
}

impl ConfigLoader for ConfigLoaderAdapter<'_> {
    fn load(&self) -> Result<Config, ConfigLoadError> {
        self.inner.load().map_err(map_config_error)
    }

    fn save(&self, config: &Config) -> Result<(), ConfigLoadError> {
        self.inner.save(config).map_err(map_config_error)
    }

    fn exists(&self) -> bool {
        self.inner.config_exists()
    }
}

fn map_config_error(e: ConfigError) -> ConfigLoadError {
    match e {
        ConfigError::InvalidConfig(msg) => ConfigLoadError::Invalid(msg),
        other => ConfigLoadError::IoFailed(other.into()),
    }
}
