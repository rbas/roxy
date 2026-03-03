use std::net::Ipv4Addr;

use anyhow::Result;

use crate::domain::DomainRegistration;

use super::ports::{CertificateManager, DaemonControl, DomainRepository, NetworkInfo};

/// Snapshot of the daemon's current status.
pub struct DaemonStatus {
    pub pid: Option<u32>,
    pub lan_ip: Ipv4Addr,
    pub ca_installed: bool,
    pub domains: Vec<DomainRegistration>,
}

/// Application service for querying daemon status.
pub struct QueryDaemonStatus<'a> {
    daemon: &'a dyn DaemonControl,
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
    network: &'a dyn NetworkInfo,
}

impl<'a> QueryDaemonStatus<'a> {
    pub fn new(
        daemon: &'a dyn DaemonControl,
        domains: &'a dyn DomainRepository,
        certs: &'a dyn CertificateManager,
        network: &'a dyn NetworkInfo,
    ) -> Self {
        Self {
            daemon,
            domains,
            certs,
            network,
        }
    }

    /// Collect the current daemon status.
    pub fn execute(&self) -> Result<DaemonStatus> {
        let pid = self.daemon.get_running_pid()?;
        let lan_ip = self.network.lan_ip().unwrap_or(Ipv4Addr::LOCALHOST);
        let ca_installed = self.certs.is_ca_installed().unwrap_or(false);
        let domains = self.domains.list().unwrap_or_default();

        Ok(DaemonStatus {
            pid,
            lan_ip,
            ca_installed,
            domains,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::testkit::*;
    use crate::domain::{DomainPattern, PathPrefix, ProxyTarget, Route, RouteTarget};

    fn registration(name: &str) -> DomainRegistration {
        DomainRegistration::new(
            DomainPattern::from_name(name, false).unwrap(),
            vec![Route::new(
                PathPrefix::new("/").unwrap(),
                RouteTarget::Proxy(ProxyTarget::parse("3000").unwrap()),
            )],
        )
    }

    #[test]
    fn reports_running_daemon_with_domains() {
        let daemon = InMemoryDaemonControl::running(42);
        let repo = InMemoryDomainRepository::with_domains(vec![registration("myapp.roxy")]);
        let certs = InMemoryCertificateManager::with_ca_installed();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::new(10, 0, 0, 1));
        let svc = QueryDaemonStatus::new(&daemon, &repo, &certs, &network);

        let status = svc.execute().unwrap();

        assert_eq!(status.pid, Some(42));
        assert_eq!(status.lan_ip, Ipv4Addr::new(10, 0, 0, 1));
        assert!(status.ca_installed);
        assert_eq!(status.domains.len(), 1);
    }

    #[test]
    fn reports_stopped_daemon() {
        let daemon = InMemoryDaemonControl::stopped();
        let repo = InMemoryDomainRepository::new();
        let certs = InMemoryCertificateManager::new();
        let network = InMemoryNetworkInfo::unavailable();
        let svc = QueryDaemonStatus::new(&daemon, &repo, &certs, &network);

        let status = svc.execute().unwrap();

        assert_eq!(status.pid, None);
        assert_eq!(status.lan_ip, Ipv4Addr::LOCALHOST);
        assert!(!status.ca_installed);
        assert!(status.domains.is_empty());
    }
}
