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
