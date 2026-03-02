use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::install::Install;
use crate::infrastructure::adapters::{
    CertificateAdapter, ConfigLoaderAdapter, DnsAdapter, NetworkInfoAdapter, SystemSetupAdapter,
};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::{Config, ConfigStore};
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::network::get_network_info;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(config_path: &Path, paths: &RoxyPaths, config: &Config) -> Result<()> {
    println!("Setting up Roxy...\n");

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let dns_service = get_dns_service()?;

    let certs = CertificateAdapter::new(&cert_service);
    let config_loader = ConfigLoaderAdapter::new(&config_store);
    let dns = DnsAdapter::new(dns_service);
    let network = NetworkInfoAdapter::new(get_network_info());
    let system = SystemSetupAdapter::new(paths);

    let use_case = Install::new(&certs, &config_loader, &dns, &network, &system, config);
    let result = use_case.execute()?;

    println!("  Using IP address: {}", result.lan_ip);
    if result.lan_ip.is_loopback() {
        println!("  Warning: No network detected, using localhost.");
    }

    for (label, outcome) in &result.steps {
        match outcome {
            StepOutcome::Success(msg) => println!("  {} {}", label, msg),
            StepOutcome::Warning(msg) => eprintln!("  {} {}", label, msg),
            StepOutcome::Skipped(msg) => println!("  {} {}", label, msg),
        }
    }

    println!("\nRoxy installation complete!");
    println!();
    println!("Register domains with: roxy register <domain> --port <port>");

    Ok(())
}
