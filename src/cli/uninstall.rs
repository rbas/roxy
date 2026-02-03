use anyhow::Result;

use crate::infrastructure::dns::{DnsService, get_dns_service};

pub fn execute(force: bool) -> Result<()> {
    if !force {
        println!("This will remove all Roxy configuration including:");
        println!("  - DNS configuration for *.roxy domains");
        println!("  - All registered domains (future)");
        println!("  - All SSL certificates (future)");
        println!("\nRun with --force to confirm, or press Ctrl+C to cancel.");
        return Ok(());
    }

    println!("Uninstalling Roxy...\n");

    let dns = get_dns_service()?;

    if dns.is_configured() {
        println!("  Removing DNS configuration...");
        dns.cleanup()?;
        println!("  DNS configuration removed.");
    } else {
        println!("  DNS not configured, skipping...");
    }

    println!("\nRoxy uninstallation complete!");

    Ok(())
}
