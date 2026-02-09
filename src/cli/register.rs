use std::path::Path;

use anyhow::{Result, bail};

use crate::domain::{DomainName, DomainRegistration, Route};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(
    domain: String,
    routes: Vec<String>,
    config_path: &Path,
    paths: &RoxyPaths,
) -> Result<()> {
    // Validate domain name
    let domain = DomainName::new(&domain)?;

    // Parse routes
    if routes.is_empty() {
        bail!("At least one route is required. Use --route \"/=PORT\" or --route \"/=PATH\"");
    }

    let parsed_routes: Vec<Route> = routes
        .iter()
        .map(|s| Route::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid route: {}", e))?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);

    // Check if already registered
    if config_store.get_domain(&domain)?.is_some() {
        bail!(
            "Domain '{}' is already registered. Use 'roxy unregister {}' first.",
            domain,
            domain
        );
    }

    // Create registration
    let mut registration = DomainRegistration::new(domain.clone(), parsed_routes);

    // Generate and install certificate
    println!("Generating SSL certificate for {}...", domain);
    match cert_service.create_and_install(&domain) {
        Ok(()) => {
            registration.enable_https();
            println!("  Certificate installed and trusted.");
        }
        Err(e) => {
            // Certificate generation failed - warn but continue
            eprintln!("  Warning: Failed to generate certificate: {}", e);
            eprintln!("  HTTPS will not be available for this domain.");
            eprintln!("  Run 'sudo roxy register {}' to enable HTTPS.", domain);
        }
    }

    // Save to config
    config_store.add_domain(registration.clone())?;

    println!("\nRegistered domain: {}", domain);
    println!("  Routes:");
    for route in &registration.routes {
        println!("    {} -> {}", route.path, route.target);
    }
    println!(
        "  HTTPS: {}",
        if cert_service.exists(&domain) {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\nStart the proxy with: roxy start");

    Ok(())
}
