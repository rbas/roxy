use anyhow::{Result, bail};

use crate::domain::DomainName;
use crate::infrastructure::config::ConfigStore;

pub fn execute(domain: String, force: bool) -> Result<()> {
    let domain = DomainName::new(&domain)?;

    let config_store = ConfigStore::new();

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
        println!("\nRun with --force to confirm.");
        return Ok(());
    }

    // Remove from config
    config_store.remove_domain(&domain)?;

    println!("Unregistered domain: {}", domain);

    // TODO: Remove SSL certificate when cert management is implemented

    Ok(())
}
