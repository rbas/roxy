use anyhow::Result;

use crate::domain::Target;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

pub fn execute() -> Result<()> {
    let config_store = ConfigStore::new();
    let cert_service = CertificateService::new();
    let domains = config_store.list_domains()?;

    if domains.is_empty() {
        println!("No domains registered.");
        println!("\nRegister a domain with:");
        println!("  roxy register myapp.roxy --port 3000");
        println!("  roxy register static.roxy --path ./public");
        return Ok(());
    }

    println!("Registered domains:\n");
    println!(
        "{:<25} {:<10} {:<30} {:<8}",
        "DOMAIN", "TYPE", "TARGET", "HTTPS"
    );
    println!("{}", "-".repeat(75));

    for reg in domains {
        let (dtype, target) = match &reg.target {
            Target::Path(p) => ("path", p.display().to_string()),
            Target::Port(p) => ("port", format!("localhost:{}", p)),
        };

        // Check actual certificate status
        let https_status = if cert_service.exists(&reg.domain) {
            match cert_service.is_trusted(&reg.domain) {
                Ok(true) => "yes",
                Ok(false) => "untrusted",
                Err(_) => "error",
            }
        } else {
            "no"
        };

        println!(
            "{:<25} {:<10} {:<30} {:<8}",
            reg.domain, dtype, target, https_status
        );
    }

    println!("\n// Daemon status will be shown here in a future update");

    Ok(())
}
