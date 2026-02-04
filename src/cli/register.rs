use anyhow::{Result, bail};
use std::path::PathBuf;

use crate::domain::{DomainName, DomainRegistration, Target};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

pub fn execute(domain: String, path: Option<PathBuf>, port: Option<u16>) -> Result<()> {
    // Validate domain name
    let domain = DomainName::new(&domain)?;

    // Determine target
    let target = match (path, port) {
        (Some(p), None) => Target::path(p)?,
        (None, Some(p)) => Target::port(p)?,
        (Some(_), Some(_)) => bail!("Cannot specify both --path and --port"),
        (None, None) => bail!("Must specify either --path or --port"),
    };

    let config_store = ConfigStore::new();
    let cert_service = CertificateService::new();

    // Check if already registered
    if config_store.get_domain(&domain)?.is_some() {
        bail!(
            "Domain '{}' is already registered. Use 'roxy unregister {}' first.",
            domain,
            domain
        );
    }

    // Create registration
    let mut registration = DomainRegistration::new(domain.clone(), target.clone());

    // Generate and install certificate
    println!("Generating SSL certificate for {}...", domain);
    match cert_service.create_and_install(&domain) {
        Ok(()) => {
            registration.enable_https();
            println!("  Certificate installed and trusted.");
        }
        Err(e) => {
            // Certificate generation failed - warn but continue
            eprintln!("  Warning: Failed to generate certificate: {}", e);
            eprintln!("  HTTPS will not be available for this domain.");
            eprintln!("  Run 'sudo roxy register {}' to enable HTTPS.", domain);
        }
    }

    // Save to config
    config_store.add_domain(registration)?;

    println!("\nRegistered domain: {}", domain);
    println!("  Target: {}", target);
    println!(
        "  HTTPS: {}",
        if cert_service.exists(&domain) {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\nStart the proxy with: roxy start");

    Ok(())
}
