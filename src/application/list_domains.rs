use anyhow::Result;

use crate::domain::DomainRegistration;

use super::ports::{CertificateManager, DomainRepository};

/// Extended domain info for display purposes.
pub struct DomainInfo {
    pub registration: DomainRegistration,
    pub has_cert: bool,
    pub cert_trusted: Option<bool>,
}

/// Use case: list all registered domains with certificate status.
pub struct ListDomains<'a> {
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
}

impl<'a> ListDomains<'a> {
    pub fn new(domains: &'a dyn DomainRepository, certs: &'a dyn CertificateManager) -> Self {
        Self { domains, certs }
    }

    /// Return all registered domains with certificate information.
    pub fn execute(&self) -> Result<Vec<DomainInfo>> {
        let registrations = self.domains.list()?;
        let cert_trusted = self.certs.is_trusted().ok();

        let infos = registrations
            .into_iter()
            .map(|reg| {
                let has_cert = self.certs.exists(reg.pattern());
                DomainInfo {
                    registration: reg,
                    has_cert,
                    cert_trusted,
                }
            })
            .collect();

        Ok(infos)
    }
}
