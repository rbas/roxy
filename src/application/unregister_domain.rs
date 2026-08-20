use anyhow::{Result, anyhow};

use crate::domain::{DomainPattern, DomainRegistration};

use super::ports::DomainRepository;

/// Result of a successful domain unregistration.
pub struct UnregisterResult {
    pub registration: DomainRegistration,
}

/// Use case: unregister a domain and clean up its certificate.
pub struct UnregisterDomain<'a> {
    domains: &'a dyn DomainRepository,
}

impl<'a> UnregisterDomain<'a> {
    pub fn new(domains: &'a dyn DomainRepository) -> Self {
        Self { domains }
    }

    /// Look up the registration so the CLI can show a confirmation
    /// prompt before proceeding with `execute()`.
    pub fn preview(&self, pattern: &DomainPattern) -> Result<DomainRegistration> {
        self.domains
            .get(pattern)?
            .ok_or_else(|| anyhow!("Domain '{}' is not registered.", pattern))
    }

    /// Remove the domain config entry.
    pub fn execute(&self, pattern: &DomainPattern) -> Result<UnregisterResult> {
        let registration = self.preview(pattern)?;

        self.domains.remove(pattern)?;

        Ok(UnregisterResult { registration })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    #[test]
    fn unregisters_domain() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = UnregisterDomain::new(&repo);

        svc.execute(&exact("myapp.roxy")).unwrap();

        assert!(repo.get(&exact("myapp.roxy")).unwrap().is_none());
    }

    #[test]
    fn fails_for_unknown_domain() {
        let repo = InMemoryDomainRepository::new();
        let svc = UnregisterDomain::new(&repo);

        let err = svc.execute(&exact("unknown.roxy")).err().unwrap();
        assert!(err.to_string().contains("not registered"));
    }

    #[test]
    fn preview_returns_registration() {
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let svc = UnregisterDomain::new(&repo);

        let reg = svc.preview(&exact("myapp.roxy")).unwrap();
        assert_eq!(reg.domain().as_str(), "myapp.roxy");
    }
}
