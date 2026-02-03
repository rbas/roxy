use super::{DomainName, Target};
use serde::{Deserialize, Serialize};

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
}
