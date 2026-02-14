//! Persistence DTO for `DomainRegistration`.
//!
//! Decouples the on-disk TOML format from the domain entity so that
//! adding or removing domain fields doesn't accidentally change the
//! config file layout, and deserialization can't bypass domain
//! invariants enforced by `DomainRegistration` methods.

use serde::{Deserialize, Serialize};

use crate::domain::{DomainPattern, DomainRegistration, Route};

/// Serializable representation of a domain registration in the config
/// file. Converted to/from `DomainRegistration` at the `ConfigStore`
/// boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationDto {
    pub pattern: DomainPattern,
    pub routes: Vec<Route>,
    #[serde(default)]
    pub https_enabled: bool,
}

impl From<DomainRegistration> for RegistrationDto {
    fn from(reg: DomainRegistration) -> Self {
        Self {
            pattern: reg.pattern().clone(),
            routes: reg.routes().to_vec(),
            https_enabled: reg.is_https_enabled(),
        }
    }
}

impl From<RegistrationDto> for DomainRegistration {
    fn from(dto: RegistrationDto) -> Self {
        let mut reg = DomainRegistration::new(dto.pattern, dto.routes);
        if dto.https_enabled {
            reg.enable_https();
        }
        reg
    }
}
