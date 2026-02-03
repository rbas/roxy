use anyhow::Result;

use crate::infrastructure::dns::{DnsService, get_dns_service};

pub fn execute() -> Result<()> {
    println!("Setting up Roxy...\n");

    let dns = get_dns_service()?;

    // Step 1: Check if already configured
    if dns.is_configured() {
        println!("  DNS already configured, skipping...");
    } else {
        println!("  Configuring DNS for *.roxy domains...");
        dns.setup()?;
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
