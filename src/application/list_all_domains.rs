use anyhow::Result;
use tracing::warn;

use crate::domain::DomainRegistration;
#[cfg(test)]
use crate::domain::RegistrationSource;

use super::ports::{CertificateManager, DaemonConnection, DaemonConnectionError, DomainRepository};

/// Extended domain info for display purposes, including source awareness.
pub struct DomainInfo {
    pub registration: DomainRegistration,
    pub has_cert: bool,
    pub cert_trusted: Option<bool>,
}

/// Result of listing all domains.
pub struct ListResult {
    pub domains: Vec<DomainInfo>,
    /// Whether the daemon was reachable (useful for showing Docker hints).
    pub daemon_reachable: bool,
}

/// Use case: list all domains from daemon (config + Docker) with
/// fallback to config-only when daemon is not running.
pub struct ListAllDomains<'a> {
    daemon: &'a dyn DaemonConnection,
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
}

impl<'a> ListAllDomains<'a> {
    pub fn new(
        daemon: &'a dyn DaemonConnection,
        domains: &'a dyn DomainRepository,
        certs: &'a dyn CertificateManager,
    ) -> Self {
        Self {
            daemon,
            domains,
            certs,
        }
    }

    /// List all domains, trying the daemon first for the full picture
    /// (config + Docker), falling back to config-only if daemon is not running.
    pub fn execute(&self) -> Result<ListResult> {
        let cert_trusted = self.certs.is_trusted().ok();
        let https_available = cert_trusted.unwrap_or(false);

        // Try daemon first — it has config + Docker domains
        match self.daemon.list_registrations() {
            Ok(regs) => {
                let domains = regs
                    .into_iter()
                    .map(|reg| DomainInfo {
                        registration: reg,
                        has_cert: https_available,
                        cert_trusted,
                    })
                    .collect();

                return Ok(ListResult {
                    domains,
                    daemon_reachable: true,
                });
            }
            Err(DaemonConnectionError::NotRunning) => {
                // Expected when daemon is stopped — fall through to config
            }
            Err(e) => {
                warn!("Failed to query daemon: {e}");
            }
        }

        // Fallback: config file only
        let regs = self.domains.list()?;
        let domains = regs
            .into_iter()
            .map(|reg| DomainInfo {
                registration: reg,
                has_cert: https_available,
                cert_trusted,
            })
            .collect();

        Ok(ListResult {
            domains,
            daemon_reachable: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;

    fn external_registration(name: &str) -> DomainRegistration {
        DomainRegistration::with_source(
            exact(name),
            vec![proxy_route("/", 3000)],
            RegistrationSource::External,
        )
    }

    #[test]
    fn uses_daemon_when_available() {
        let daemon = InMemoryDaemonConnection::new(vec![
            registration("app.roxy"),
            external_registration("docker-app.roxy"),
        ]);
        let repo = InMemoryDomainRepository::with_domains(vec![registration("app.roxy")]);
        let certs = InMemoryCertificateManager::with_ca_installed();

        let svc = ListAllDomains::new(&daemon, &repo, &certs);
        let result = svc.execute().unwrap();

        assert!(result.daemon_reachable);
        assert_eq!(result.domains.len(), 2);

        let docker = result
            .domains
            .iter()
            .find(|d| d.registration.source() == RegistrationSource::External);
        assert!(docker.is_some());
    }

    #[test]
    fn falls_back_to_config_when_daemon_not_running() {
        let daemon = NotRunningDaemonConnection;
        let repo = InMemoryDomainRepository::with_domains(vec![registration("app.roxy")]);
        let certs = InMemoryCertificateManager::new();

        let svc = ListAllDomains::new(&daemon, &repo, &certs);
        let result = svc.execute().unwrap();

        assert!(!result.daemon_reachable);
        assert_eq!(result.domains.len(), 1);
    }

    #[test]
    fn enriches_with_cert_info() {
        let daemon = InMemoryDaemonConnection::new(vec![registration("app.roxy")]);
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::with_ca_installed();

        let svc = ListAllDomains::new(&daemon, &repo, &certs);
        let result = svc.execute().unwrap();

        assert_eq!(result.domains.len(), 1);
        assert!(result.domains[0].has_cert);
        assert_eq!(result.domains[0].cert_trusted, Some(true));
    }
}
