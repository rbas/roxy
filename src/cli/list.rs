use std::path::Path;

use anyhow::Result;

use crate::domain::RouteTarget;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
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
        let has_cert = if reg.wildcard {
            cert_service.exists_wildcard(&reg.domain)
        } else {
            cert_service.exists(&reg.domain)
        };
        let https_status = if has_cert {
            match cert_service.is_trusted(&reg.domain) {
                Ok(true) => "(HTTPS)",
                Ok(false) => "(HTTPS untrusted)",
                Err(_) => "(HTTPS error)",
            }
        } else {
            ""
        };

        println!("  {} {}", reg.display_pattern(), https_status);

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
