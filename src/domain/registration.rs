use super::{DomainName, Target};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("Target path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Target path is not a directory: {0}")]
    NotADirectory(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainRegistration {
    pub domain: DomainName,
    pub target: Target,
    pub https_enabled: bool,
}

impl DomainRegistration {
    pub fn new(domain: DomainName, target: Target) -> Self {
        Self {
            domain,
            target,
            https_enabled: false, // Will be enabled after cert generation
        }
    }

    #[allow(dead_code)] // Will be used when certificate management is implemented
    pub fn enable_https(&mut self) {
        self.https_enabled = true;
    }

    /// Validate that the registration is still valid (e.g., paths exist)
    pub fn validate(&self) -> Result<(), RegistrationError> {
        match &self.target {
            Target::Path(path) => {
                if !path.exists() {
                    return Err(RegistrationError::PathNotFound(path.clone()));
                }
                if !path.is_dir() {
                    return Err(RegistrationError::NotADirectory(path.clone()));
                }
            }
            Target::Port(_) => {
                // Port targets don't need validation - the service may not be running yet
            }
        }
        Ok(())
    }
}
