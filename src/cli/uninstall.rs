use anyhow::Result;

use super::context::AppContext;
use crate::application::StepOutcome;
use crate::application::uninstall::Uninstall;
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::filesystem::FileSystemSetup;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::service;

pub fn execute(force: bool, ctx: &AppContext, paths: &RoxyPaths) -> Result<()> {
    let dns_service = get_dns_service()?;
    let system = FileSystemSetup::new(paths);

    let use_case = Uninstall::new(
        &ctx.config_store,
        &ctx.cert_service,
        &ctx.pid_file,
        &dns_service,
        &system,
        paths.data_dir.display().to_string(),
    );

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

    service::uninstall()?;
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
