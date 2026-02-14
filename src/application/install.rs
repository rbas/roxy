use std::net::Ipv4Addr;
use std::path::Path;

use anyhow::{Context, Result};

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::{Config, ConfigStore};
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;

use super::StepOutcome;

/// Result of the install operation.
pub struct InstallResult {
    pub lan_ip: Ipv4Addr,
    pub steps: Vec<(String, StepOutcome)>,
}

/// Use case: initial setup — create directories, root CA, DNS.
pub struct Install<'a> {
    config_store: &'a ConfigStore,
    cert_service: &'a CertificateService,
    config_path: &'a Path,
    paths: &'a RoxyPaths,
    config: &'a Config,
}

impl<'a> Install<'a> {
    pub fn new(
        config_store: &'a ConfigStore,
        cert_service: &'a CertificateService,
        config_path: &'a Path,
        paths: &'a RoxyPaths,
        config: &'a Config,
    ) -> Self {
        Self {
            config_store,
            cert_service,
            config_path,
            paths,
            config,
        }
    }

    pub fn execute(&self) -> Result<InstallResult> {
        let mut steps: Vec<(String, StepOutcome)> = Vec::new();
        let lan_ip = get_lan_ip();
        let dns_port = self.config.daemon.dns_port;

        self.create_directories(&mut steps)?;
        self.ensure_config_file(&mut steps)?;
        self.init_root_ca(&mut steps);
        self.configure_dns(dns_port, &mut steps)?;

        Ok(InstallResult { lan_ip, steps })
    }

    fn create_directories(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        std::fs::create_dir_all(&self.paths.data_dir).with_context(|| {
            format!(
                "Failed to create data directory: {}",
                self.paths.data_dir.display()
            )
        })?;
        std::fs::create_dir_all(&self.paths.certs_dir).with_context(|| {
            format!(
                "Failed to create certs directory: {}",
                self.paths.certs_dir.display()
            )
        })?;

        if let Some(log_dir) = self.paths.log_file.parent() {
            std::fs::create_dir_all(log_dir).with_context(|| {
                format!("Failed to create log directory: {}", log_dir.display())
            })?;
        }

        steps.push((
            "Create directories".into(),
            StepOutcome::Success("Data and log directories ready.".into()),
        ));
        Ok(())
    }

    fn ensure_config_file(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        if !self.config_path.exists() {
            self.config_store.save(self.config)?;
            steps.push((
                "Config file".into(),
                StepOutcome::Success(format!(
                    "Created config file: {}",
                    self.config_path.display()
                )),
            ));
        } else {
            steps.push((
                "Config file".into(),
                StepOutcome::Skipped("Config file already exists.".into()),
            ));
        }
        Ok(())
    }

    fn init_root_ca(&self, steps: &mut Vec<(String, StepOutcome)>) {
        let ca_outcome = match self.cert_service.is_ca_installed() {
            Ok(true) => StepOutcome::Skipped("Root CA already installed.".into()),
            _ => match self.cert_service.init_ca() {
                Ok(()) => StepOutcome::Success(
                    "Root CA created and installed in system trust store.".into(),
                ),
                Err(e) => StepOutcome::Warning(format!(
                    "Failed to create Root CA: {}. \
                     HTTPS certificates will not work. \
                     Run 'sudo roxy install' to enable HTTPS.",
                    e
                )),
            },
        };
        steps.push(("Root CA".into(), ca_outcome));
    }

    fn configure_dns(&self, dns_port: u16, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        let dns = get_dns_service()?;
        let dns_outcome = if dns.is_configured() {
            StepOutcome::Skipped("DNS already configured.".into())
        } else {
            dns.setup(dns_port)?;
            StepOutcome::Success("DNS configured successfully.".into())
        };
        steps.push(("DNS configuration".into(), dns_outcome));

        dns.validate()?;
        steps.push((
            "DNS validation".into(),
            StepOutcome::Success("DNS validation passed.".into()),
        ));
        Ok(())
    }
}
