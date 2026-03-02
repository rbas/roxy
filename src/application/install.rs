use std::net::Ipv4Addr;

use anyhow::Result;

use crate::infrastructure::config::Config;

use super::StepOutcome;
use super::ports::{CertificateManager, ConfigLoader, DnsManager, NetworkInfo, SystemSetup};

/// Result of the install operation.
pub struct InstallResult {
    pub lan_ip: Ipv4Addr,
    pub steps: Vec<(String, StepOutcome)>,
}

/// Use case: initial setup — create directories, root CA, DNS.
pub struct Install<'a> {
    certs: &'a dyn CertificateManager,
    config_loader: &'a dyn ConfigLoader,
    dns: &'a dyn DnsManager,
    network: &'a dyn NetworkInfo,
    system: &'a dyn SystemSetup,
    config: &'a Config,
}

impl<'a> Install<'a> {
    pub fn new(
        certs: &'a dyn CertificateManager,
        config_loader: &'a dyn ConfigLoader,
        dns: &'a dyn DnsManager,
        network: &'a dyn NetworkInfo,
        system: &'a dyn SystemSetup,
        config: &'a Config,
    ) -> Self {
        Self {
            certs,
            config_loader,
            dns,
            network,
            system,
            config,
        }
    }

    pub fn execute(&self) -> Result<InstallResult> {
        let mut steps: Vec<(String, StepOutcome)> = Vec::new();
        let lan_ip = self.network.lan_ip().unwrap_or(Ipv4Addr::LOCALHOST);
        let dns_port = self.config.daemon.dns_port;

        self.create_directories(&mut steps)?;
        self.ensure_config_file(&mut steps)?;
        self.init_root_ca(&mut steps);
        self.configure_dns(dns_port, &mut steps)?;

        Ok(InstallResult { lan_ip, steps })
    }

    fn create_directories(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        self.system.create_directories()?;
        steps.push((
            "Create directories".into(),
            StepOutcome::Success("Data and log directories ready.".into()),
        ));
        Ok(())
    }

    fn ensure_config_file(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        if !self.config_loader.exists() {
            self.config_loader.save(self.config)?;
            steps.push((
                "Config file".into(),
                StepOutcome::Success("Created config file.".into()),
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
        let ca_outcome = match self.certs.is_ca_installed() {
            Ok(true) => StepOutcome::Skipped("Root CA already installed.".into()),
            _ => match self.certs.init_ca() {
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
        let dns_outcome = if self.dns.is_configured() {
            StepOutcome::Skipped("DNS already configured.".into())
        } else {
            self.dns.setup(dns_port)?;
            StepOutcome::Success("DNS configured successfully.".into())
        };
        steps.push(("DNS configuration".into(), dns_outcome));

        self.dns.validate()?;
        steps.push((
            "DNS validation".into(),
            StepOutcome::Success("DNS validation passed.".into()),
        ));
        Ok(())
    }
}
