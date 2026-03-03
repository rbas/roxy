use std::path::Path;

use anyhow::Result;

use crate::application::list_domains::ListDomains;
use crate::domain::RouteTarget;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);

    let use_case = ListDomains::new(&config_store, &cert_service);
    let domains = use_case.execute()?;

    if domains.is_empty() {
        println!("No domains registered.");
        println!("\nRegister a domain with:");
        println!("  roxy register myapp.roxy --route \"/=3000\"");
        println!("  roxy register myapp.roxy --route \"/=3000\" --route \"/api=3001\"");
        return Ok(());
    }

    println!("Registered domains:\n");

    for info in domains {
        let https_status = if info.has_cert {
            match info.cert_trusted {
                Some(true) => "(HTTPS)",
                Some(false) => "(HTTPS untrusted)",
                None => "(HTTPS error)",
            }
        } else {
            ""
        };

        println!("  {} {}", info.registration.display_pattern(), https_status);

        for route in info.registration.routes() {
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
