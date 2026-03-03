//! In-memory implementations of all application ports for testing.
//!
//! These are real implementations backed by simple data structures.
//! No mock libraries, no magic — just `RefCell<Vec<..>>` and friends.

use std::cell::RefCell;
use std::net::Ipv4Addr;
use std::time::Duration;

use crate::config::{DaemonConfig, RoxyPaths};
use crate::domain::{DomainPattern, DomainRegistration};

use super::ports::{
    CertificateError, CertificateManager, ConfigLoadError, ConfigLoader, DaemonConnection,
    DaemonConnectionError, DaemonControl, DaemonStatus, DnsConfigError, DnsManager,
    DomainRepository, NetworkInfo, RegistrationProvider, RepositoryError, SystemSetup,
};

// ---------------------------------------------------------------------------
// InMemoryRegistrationProvider
// ---------------------------------------------------------------------------

pub struct InMemoryRegistrationProvider {
    registrations: Vec<DomainRegistration>,
}

impl InMemoryRegistrationProvider {
    pub fn new(registrations: Vec<DomainRegistration>) -> Self {
        Self { registrations }
    }
}

impl RegistrationProvider for InMemoryRegistrationProvider {
    fn name(&self) -> &str {
        "in-memory"
    }

    fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
        Ok(self.registrations.clone())
    }
}

// ---------------------------------------------------------------------------
// InMemoryDomainRepository
// ---------------------------------------------------------------------------

pub struct InMemoryDomainRepository {
    domains: RefCell<Vec<DomainRegistration>>,
}

impl InMemoryDomainRepository {
    pub fn new() -> Self {
        Self {
            domains: RefCell::new(Vec::new()),
        }
    }

    pub fn with_domains(domains: Vec<DomainRegistration>) -> Self {
        Self {
            domains: RefCell::new(domains),
        }
    }
}

impl DomainRepository for InMemoryDomainRepository {
    fn get(&self, pattern: &DomainPattern) -> Result<Option<DomainRegistration>, RepositoryError> {
        Ok(self
            .domains
            .borrow()
            .iter()
            .find(|r| r.pattern() == pattern)
            .cloned())
    }

    fn list(&self) -> Result<Vec<DomainRegistration>, RepositoryError> {
        Ok(self.domains.borrow().clone())
    }

    fn add(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        let key = registration.config_key();
        if self.domains.borrow().iter().any(|r| r.config_key() == key) {
            return Err(RepositoryError::DomainExists(key));
        }
        self.domains.borrow_mut().push(registration);
        Ok(())
    }

    fn update(&self, registration: DomainRegistration) -> Result<(), RepositoryError> {
        let key = registration.config_key();
        let mut domains = self.domains.borrow_mut();
        let pos = domains
            .iter()
            .position(|r| r.config_key() == key)
            .ok_or_else(|| RepositoryError::DomainNotFound(key))?;
        domains[pos] = registration;
        Ok(())
    }

    fn remove(&self, pattern: &DomainPattern) -> Result<(), RepositoryError> {
        let key = pattern.display_pattern();
        let mut domains = self.domains.borrow_mut();
        let pos = domains
            .iter()
            .position(|r| r.config_key() == key)
            .ok_or_else(|| RepositoryError::DomainNotFound(key))?;
        domains.remove(pos);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InMemoryCertificateManager
// ---------------------------------------------------------------------------

pub struct InMemoryCertificateManager {
    ca_installed: RefCell<bool>,
    certs: RefCell<Vec<String>>,
    /// When true, all cert operations fail.
    fail_operations: bool,
}

impl InMemoryCertificateManager {
    pub fn new() -> Self {
        Self {
            ca_installed: RefCell::new(false),
            certs: RefCell::new(Vec::new()),
            fail_operations: false,
        }
    }

    pub fn always_failing() -> Self {
        Self {
            ca_installed: RefCell::new(false),
            certs: RefCell::new(Vec::new()),
            fail_operations: true,
        }
    }

    pub fn with_ca_installed() -> Self {
        Self {
            ca_installed: RefCell::new(true),
            certs: RefCell::new(Vec::new()),
            fail_operations: false,
        }
    }
}

impl CertificateManager for InMemoryCertificateManager {
    fn init_ca(&self) -> Result<(), CertificateError> {
        if self.fail_operations {
            return Err(CertificateError::OperationFailed(anyhow::anyhow!(
                "simulated CA failure"
            )));
        }
        *self.ca_installed.borrow_mut() = true;
        Ok(())
    }

    fn is_ca_installed(&self) -> Result<bool, CertificateError> {
        Ok(*self.ca_installed.borrow())
    }

    fn create_and_install(&self, pattern: &DomainPattern) -> Result<(), CertificateError> {
        if self.fail_operations {
            return Err(CertificateError::OperationFailed(anyhow::anyhow!(
                "simulated cert failure"
            )));
        }
        self.certs.borrow_mut().push(pattern.cert_name());
        Ok(())
    }

    fn remove(&self, pattern: &DomainPattern) -> Result<(), CertificateError> {
        if self.fail_operations {
            return Err(CertificateError::OperationFailed(anyhow::anyhow!(
                "simulated remove failure"
            )));
        }
        self.certs
            .borrow_mut()
            .retain(|c| *c != pattern.cert_name());
        Ok(())
    }

    fn remove_ca(&self) -> Result<(), CertificateError> {
        if self.fail_operations {
            return Err(CertificateError::OperationFailed(anyhow::anyhow!(
                "simulated remove-CA failure"
            )));
        }
        *self.ca_installed.borrow_mut() = false;
        Ok(())
    }

    fn exists(&self, pattern: &DomainPattern) -> bool {
        self.certs.borrow().contains(&pattern.cert_name())
    }

    fn is_trusted(&self) -> Result<bool, CertificateError> {
        Ok(*self.ca_installed.borrow())
    }
}

// ---------------------------------------------------------------------------
// InMemoryConfigLoader
// ---------------------------------------------------------------------------

pub struct InMemoryConfigLoader {
    file_exists: RefCell<bool>,
    daemon_config: DaemonConfig,
    paths: RoxyPaths,
}

impl InMemoryConfigLoader {
    pub fn new() -> Self {
        Self {
            file_exists: RefCell::new(false),
            daemon_config: DaemonConfig::default(),
            paths: RoxyPaths::default(),
        }
    }

    pub fn existing() -> Self {
        Self {
            file_exists: RefCell::new(true),
            daemon_config: DaemonConfig::default(),
            paths: RoxyPaths::default(),
        }
    }
}

impl ConfigLoader for InMemoryConfigLoader {
    fn load(&self) -> Result<(DaemonConfig, RoxyPaths), ConfigLoadError> {
        Ok((self.daemon_config.clone(), self.paths.clone()))
    }

    fn save_defaults(&self) -> Result<(), ConfigLoadError> {
        *self.file_exists.borrow_mut() = true;
        Ok(())
    }

    fn exists(&self) -> bool {
        *self.file_exists.borrow()
    }
}

// ---------------------------------------------------------------------------
// InMemoryDaemonControl
// ---------------------------------------------------------------------------

pub struct InMemoryDaemonControl {
    running_pid: RefCell<Option<u32>>,
}

impl InMemoryDaemonControl {
    pub fn stopped() -> Self {
        Self {
            running_pid: RefCell::new(None),
        }
    }

    pub fn running(pid: u32) -> Self {
        Self {
            running_pid: RefCell::new(Some(pid)),
        }
    }
}

impl DaemonControl for InMemoryDaemonControl {
    fn is_running(&self) -> anyhow::Result<bool> {
        Ok(self.running_pid.borrow().is_some())
    }

    fn get_running_pid(&self) -> anyhow::Result<Option<u32>> {
        Ok(*self.running_pid.borrow())
    }

    fn stop_gracefully(&self, _timeout: Duration) -> anyhow::Result<()> {
        *self.running_pid.borrow_mut() = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// InMemoryDnsManager
// ---------------------------------------------------------------------------

pub struct InMemoryDnsManager {
    configured: RefCell<bool>,
}

impl InMemoryDnsManager {
    pub fn new() -> Self {
        Self {
            configured: RefCell::new(false),
        }
    }

    pub fn already_configured() -> Self {
        Self {
            configured: RefCell::new(true),
        }
    }
}

impl DnsManager for InMemoryDnsManager {
    fn setup(&self, _port: u16) -> Result<(), DnsConfigError> {
        *self.configured.borrow_mut() = true;
        Ok(())
    }

    fn cleanup(&self) -> Result<(), DnsConfigError> {
        *self.configured.borrow_mut() = false;
        Ok(())
    }

    fn validate(&self) -> Result<(), DnsConfigError> {
        Ok(())
    }

    fn is_configured(&self) -> bool {
        *self.configured.borrow()
    }
}

// ---------------------------------------------------------------------------
// InMemoryNetworkInfo
// ---------------------------------------------------------------------------

pub struct InMemoryNetworkInfo {
    ip: Option<Ipv4Addr>,
}

impl InMemoryNetworkInfo {
    pub fn with_ip(ip: Ipv4Addr) -> Self {
        Self { ip: Some(ip) }
    }

    pub fn unavailable() -> Self {
        Self { ip: None }
    }
}

impl NetworkInfo for InMemoryNetworkInfo {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        self.ip
    }
}

// ---------------------------------------------------------------------------
// InMemorySystemSetup
// ---------------------------------------------------------------------------

pub struct InMemorySystemSetup {
    directories_created: RefCell<bool>,
    data_exists: RefCell<bool>,
    pid_exists: RefCell<bool>,
    log_exists: RefCell<bool>,
}

impl InMemorySystemSetup {
    pub fn new() -> Self {
        Self {
            directories_created: RefCell::new(false),
            data_exists: RefCell::new(false),
            pid_exists: RefCell::new(false),
            log_exists: RefCell::new(false),
        }
    }

    pub fn with_existing_data() -> Self {
        Self {
            directories_created: RefCell::new(true),
            data_exists: RefCell::new(true),
            pid_exists: RefCell::new(true),
            log_exists: RefCell::new(true),
        }
    }
}

impl SystemSetup for InMemorySystemSetup {
    fn create_directories(&self) -> anyhow::Result<()> {
        *self.directories_created.borrow_mut() = true;
        Ok(())
    }

    fn remove_data_directory(&self) -> anyhow::Result<bool> {
        let existed = *self.data_exists.borrow();
        *self.data_exists.borrow_mut() = false;
        Ok(existed)
    }

    fn remove_pid_file(&self) -> bool {
        let existed = *self.pid_exists.borrow();
        *self.pid_exists.borrow_mut() = false;
        existed
    }

    fn remove_log_directory(&self) -> bool {
        let existed = *self.log_exists.borrow();
        *self.log_exists.borrow_mut() = false;
        existed
    }
}

// ---------------------------------------------------------------------------
// InMemoryDaemonConnection
// ---------------------------------------------------------------------------

pub struct InMemoryDaemonConnection {
    registrations: RefCell<Vec<DomainRegistration>>,
    pid: u32,
    http_port: u16,
    https_port: u16,
    dns_port: u16,
}

impl InMemoryDaemonConnection {
    pub fn new(registrations: Vec<DomainRegistration>) -> Self {
        Self {
            registrations: RefCell::new(registrations),
            pid: 1234,
            http_port: 80,
            https_port: 443,
            dns_port: 1053,
        }
    }
}

impl DaemonConnection for InMemoryDaemonConnection {
    fn status(&self) -> Result<DaemonStatus, DaemonConnectionError> {
        Ok(DaemonStatus {
            pid: self.pid,
            registrations: self.registrations.borrow().clone(),
            http_port: self.http_port,
            https_port: self.https_port,
            dns_port: self.dns_port,
        })
    }

    fn reload(&self) -> Result<(), DaemonConnectionError> {
        Ok(())
    }

    fn list_registrations(&self) -> Result<Vec<DomainRegistration>, DaemonConnectionError> {
        Ok(self.registrations.borrow().clone())
    }
}
