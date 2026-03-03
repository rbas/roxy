use std::net::Ipv4Addr;

use anyhow::Result;

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
    dns_port: u16,
}

impl<'a> Install<'a> {
    pub fn new(
        certs: &'a dyn CertificateManager,
        config_loader: &'a dyn ConfigLoader,
        dns: &'a dyn DnsManager,
        network: &'a dyn NetworkInfo,
        system: &'a dyn SystemSetup,
        dns_port: u16,
    ) -> Self {
        Self {
            certs,
            config_loader,
            dns,
            network,
            system,
            dns_port,
        }
    }

    pub fn execute(&self) -> Result<InstallResult> {
        let mut steps: Vec<(String, StepOutcome)> = Vec::new();
        let lan_ip = self.network.lan_ip().unwrap_or(Ipv4Addr::LOCALHOST);
        let dns_port = self.dns_port;

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
            self.config_loader.save_defaults()?;
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    use crate::application::testkit::*;

    fn make_install<'a>(
        certs: &'a InMemoryCertificateManager,
        config: &'a InMemoryConfigLoader,
        dns: &'a InMemoryDnsManager,
        network: &'a InMemoryNetworkInfo,
        system: &'a InMemorySystemSetup,
    ) -> Install<'a> {
        Install::new(certs, config, dns, network, system, 1053)
    }

    #[test]
    fn fresh_install_creates_everything() {
        let certs = InMemoryCertificateManager::new();
        let config = InMemoryConfigLoader::new();
        let dns = InMemoryDnsManager::new();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::new(192, 168, 1, 100));
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        assert_eq!(result.lan_ip, Ipv4Addr::new(192, 168, 1, 100));
        // Config file was created
        assert!(config.exists());
        // DNS was configured
        assert!(dns.is_configured());
        // All steps should be Success
        for (_, outcome) in &result.steps {
            assert!(
                matches!(outcome, StepOutcome::Success(_)),
                "expected Success, got: {:?}",
                outcome
            );
        }
    }

    #[test]
    fn install_skips_existing_config() {
        let certs = InMemoryCertificateManager::new();
        let config = InMemoryConfigLoader::existing();
        let dns = InMemoryDnsManager::new();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::LOCALHOST);
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        let config_step = result
            .steps
            .iter()
            .find(|(label, _)| label == "Config file")
            .unwrap();
        assert!(matches!(config_step.1, StepOutcome::Skipped(_)));
    }

    #[test]
    fn install_skips_existing_dns() {
        let certs = InMemoryCertificateManager::new();
        let config = InMemoryConfigLoader::new();
        let dns = InMemoryDnsManager::already_configured();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::LOCALHOST);
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        let dns_step = result
            .steps
            .iter()
            .find(|(label, _)| label == "DNS configuration")
            .unwrap();
        assert!(matches!(dns_step.1, StepOutcome::Skipped(_)));
    }

    #[test]
    fn install_skips_existing_ca() {
        let certs = InMemoryCertificateManager::with_ca_installed();
        let config = InMemoryConfigLoader::new();
        let dns = InMemoryDnsManager::new();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::LOCALHOST);
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        let ca_step = result
            .steps
            .iter()
            .find(|(label, _)| label == "Root CA")
            .unwrap();
        assert!(matches!(ca_step.1, StepOutcome::Skipped(_)));
    }

    #[test]
    fn install_warns_on_ca_failure() {
        let certs = InMemoryCertificateManager::always_failing();
        let config = InMemoryConfigLoader::new();
        let dns = InMemoryDnsManager::new();
        let network = InMemoryNetworkInfo::with_ip(Ipv4Addr::LOCALHOST);
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        let ca_step = result
            .steps
            .iter()
            .find(|(label, _)| label == "Root CA")
            .unwrap();
        assert!(matches!(ca_step.1, StepOutcome::Warning(_)));
    }

    #[test]
    fn install_falls_back_to_localhost() {
        let certs = InMemoryCertificateManager::new();
        let config = InMemoryConfigLoader::new();
        let dns = InMemoryDnsManager::new();
        let network = InMemoryNetworkInfo::unavailable();
        let system = InMemorySystemSetup::new();
        let svc = make_install(&certs, &config, &dns, &network, &system);

        let result = svc.execute().unwrap();

        assert_eq!(result.lan_ip, Ipv4Addr::LOCALHOST);
    }
}
