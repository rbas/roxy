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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;
    use crate::domain::{DomainPattern, PathPrefix, ProxyTarget, Route, RouteTarget};

    fn exact(name: &str) -> DomainPattern {
        DomainPattern::from_name(name, false).unwrap()
    }

    fn registration(name: &str) -> DomainRegistration {
        DomainRegistration::new(
            exact(name),
            vec![Route::new(
                PathPrefix::new("/").unwrap(),
                RouteTarget::Proxy(ProxyTarget::parse("3000").unwrap()),
            )],
        )
    }

    #[test]
    fn lists_all_domains_with_cert_info() {
        let repo = InMemoryDomainRepository::with_domains(vec![
            registration("a.roxy"),
            registration("b.roxy"),
        ]);
        let certs = InMemoryCertificateManager::with_ca_installed();
        certs.create_and_install(&exact("a.roxy")).unwrap();
        let svc = ListDomains::new(&repo, &certs);

        let infos = svc.execute().unwrap();

        assert_eq!(infos.len(), 2);
        let a_info = infos
            .iter()
            .find(|i| i.registration.domain().as_str() == "a.roxy")
            .unwrap();
        assert!(a_info.has_cert);
        assert_eq!(a_info.cert_trusted, Some(true));

        let b_info = infos
            .iter()
            .find(|i| i.registration.domain().as_str() == "b.roxy")
            .unwrap();
        assert!(!b_info.has_cert);
    }

    #[test]
    fn returns_empty_list_when_no_domains() {
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let svc = ListDomains::new(&repo, &certs);

        let infos = svc.execute().unwrap();
        assert!(infos.is_empty());
    }
}
