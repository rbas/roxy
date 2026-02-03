use anyhow::{Result, bail};
use std::path::PathBuf;

use crate::domain::{DomainName, DomainRegistration, Target};
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

    // Check if already registered
    if config_store.get_domain(&domain)?.is_some() {
        bail!(
            "Domain '{}' is already registered. Use 'roxy unregister {}' first.",
            domain,
            domain
        );
    }

    // Create registration
    let registration = DomainRegistration::new(domain.clone(), target.clone());

    // Save to config
    config_store.add_domain(registration)?;

    println!("Registered domain: {}", domain);
    println!("  Target: {}", target);
    println!("\nNote: HTTPS certificate generation will be added in a future update.");
    println!("Start the proxy with: roxy start");

    Ok(())
}
