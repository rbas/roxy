use anyhow::Result;

use crate::domain::RouteTarget;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;

pub fn execute() -> Result<()> {
    let config_store = ConfigStore::new();
    let cert_service = CertificateService::new();
    let domains = config_store.list_domains()?;

    if domains.is_empty() {
        println!("No domains registered.");
        println!("\nRegister a domain with:");
        println!("  roxy register myapp.roxy --route \"/=3000\"");
        println!("  roxy register myapp.roxy --route \"/=3000\" --route \"/api=3001\"");
        return Ok(());
    }

    println!("Registered domains:\n");

    for reg in domains {
        // Check actual certificate status
        let https_status = if cert_service.exists(&reg.domain) {
            match cert_service.is_trusted(&reg.domain) {
                Ok(true) => "(HTTPS)",
                Ok(false) => "(HTTPS untrusted)",
                Err(_) => "(HTTPS error)",
            }
        } else {
            ""
        };

        println!("  {} {}", reg.domain, https_status);

        for route in &reg.routes {
            let target_str = match &route.target {
                RouteTarget::Proxy(p) => p.to_string(),
                RouteTarget::StaticFiles(p) => p.display().to_string(),
            };
            println!("    {:<15} -> {}", route.path, target_str);
        }
        println!();
    }

    Ok(())
}
