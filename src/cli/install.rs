use anyhow::Result;

use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::dns::{DnsService, get_dns_service};

pub fn execute() -> Result<()> {
    println!("Setting up Roxy...\n");

    let config_store = ConfigStore::new();
    let config = config_store.load()?;
    let dns_port = config.daemon.dns_port;

    let dns = get_dns_service()?;

    // Step 1: Check if already configured
    if dns.is_configured() {
        println!("  DNS already configured, skipping...");
    } else {
        println!("  Configuring DNS for *.roxy domains (port {})...", dns_port);
        dns.setup(dns_port)?;
        println!("  DNS configured successfully.");
    }

    // Step 2: Validate DNS
    println!("  Validating DNS configuration...");
    dns.validate()?;
    println!("  DNS validation passed.\n");

    println!("Roxy installation complete!");
    println!("You can now register domains with: roxy register <domain> --port <port>");

    Ok(())
}
