use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::uninstall::Uninstall;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(force: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let use_case = Uninstall::new(&config_store, &cert_service, paths);

    if !force {
        let preview = use_case.preview()?;
        println!("This will remove all Roxy configuration including:");
        println!("  - Stop the running daemon");
        println!("  - DNS configuration for *.roxy domains");
        println!("  - All registered domains ({})", preview.domain_count);
        println!("  - All SSL certificates from system trust store");
        println!("  - All data in {}/", preview.data_dir);
        println!("\nRun with --force to confirm, or press Ctrl+C to cancel.");
        return Ok(());
    }

    println!("Uninstalling Roxy...\n");

    let result = use_case.execute()?;

    for (label, outcome) in &result.steps {
        match outcome {
            StepOutcome::Success(msg) => println!("  {}: {}", label, msg),
            StepOutcome::Warning(msg) => eprintln!("  {}: {}", label, msg),
            StepOutcome::Skipped(msg) => println!("  {}: {}", label, msg),
        }
    }

    println!("\nRoxy uninstallation complete!");
    println!("All configuration and certificates have been removed.");

    Ok(())
}
