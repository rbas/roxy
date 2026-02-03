use anyhow::{bail, Result};

use crate::domain::DomainName;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

pub fn execute(domain: String, force: bool) -> Result<()> {
    let domain = DomainName::new(&domain)?;

    let config_store = ConfigStore::new();
    let cert_service = CertificateService::new();

    // Check if domain exists
    let registration = config_store.get_domain(&domain)?;
    if registration.is_none() {
        bail!("Domain '{}' is not registered.", domain);
    }

    let registration = registration.unwrap();

    if !force {
        println!("This will unregister the domain:");
        println!("  Domain: {}", registration.domain);
        println!("  Target: {}", registration.target);
        if registration.https_enabled {
            println!("  HTTPS certificate will be removed from system trust store");
        }
        println!("\nRun with --force to confirm.");
        return Ok(());
    }

    // Remove certificate if exists
    if cert_service.exists(&domain) {
        println!("Removing SSL certificate...");
        match cert_service.remove(&domain) {
            Ok(()) => println!("  Certificate removed."),
            Err(e) => {
                eprintln!("  Warning: Failed to remove certificate: {}", e);
                eprintln!("  You may need to manually remove it from Keychain Access.");
            }
        }
    }

    // Remove from config
    config_store.remove_domain(&domain)?;

    println!("Unregistered domain: {}", domain);

    Ok(())
}
