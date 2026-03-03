use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::unregister_domain::UnregisterDomain;
use crate::domain::DomainPattern;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(
    domain: String,
    wildcard: bool,
    force: bool,
    config_path: &Path,
    paths: &RoxyPaths,
) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let use_case = UnregisterDomain::new(&config_store, &cert_service);

    if !force {
        let registration = use_case.preview(&pattern)?;
        println!("This will unregister the domain:");
        println!("  Domain: {}", registration.display_pattern());
        println!("  Routes:");
        for route in registration.routes() {
            println!("    {} -> {}", route.path, route.target);
        }
        if registration.is_https_enabled() {
            println!("  HTTPS certificate files will be removed");
        }
        println!("\nRun with --force to confirm.");
        return Ok(());
    }

    let result = use_case.execute(&pattern)?;

    match &result.cert_outcome {
        StepOutcome::Success(msg) => println!("{}", msg),
        StepOutcome::Warning(msg) => eprintln!("{}", msg),
        StepOutcome::Skipped(_) => {}
    }

    println!(
        "Unregistered domain: {}",
        result.registration.display_pattern()
    );

    Ok(())
}
