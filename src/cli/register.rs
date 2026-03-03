use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::register_domain::RegisterDomain;
use crate::domain::{DomainPattern, Route};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(
    domain: String,
    wildcard: bool,
    routes: Vec<String>,
    config_path: &Path,
    paths: &RoxyPaths,
) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let parsed_routes: Vec<Route> = routes
        .iter()
        .map(|s| Route::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid route: {}", e))?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let use_case = RegisterDomain::new(&config_store, &cert_service);

    println!(
        "Generating SSL certificate for {}...",
        pattern.display_pattern()
    );

    let result = use_case.execute(pattern, parsed_routes)?;

    match &result.cert_outcome {
        StepOutcome::Success(msg) => println!("  {}", msg),
        StepOutcome::Warning(msg) => {
            eprintln!("  {}", msg);
            eprintln!(
                "  Run 'sudo roxy register {}{}' to enable HTTPS.",
                result.registration.domain(),
                if wildcard { " --wildcard" } else { "" }
            );
        }
        StepOutcome::Skipped(msg) => println!("  {}", msg),
    }

    println!(
        "\nRegistered domain: {}",
        result.registration.display_pattern()
    );
    println!("  Routes:");
    for route in result.registration.routes() {
        println!("    {} -> {}", route.path, route.target);
    }
    println!(
        "  HTTPS: {}",
        if result.registration.is_https_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\nStart the proxy with: roxy start");

    Ok(())
}
