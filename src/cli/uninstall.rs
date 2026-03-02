use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::uninstall::Uninstall;
use crate::infrastructure::adapters::{
    CertificateAdapter, DaemonControlAdapter, DnsAdapter, DomainRepositoryAdapter,
    SystemSetupAdapter,
};
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(force: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let pid_file = PidFile::new(paths.pid_file.clone());
    let dns_service = get_dns_service()?;

    let repo = DomainRepositoryAdapter::new(&config_store);
    let certs = CertificateAdapter::new(&cert_service);
    let daemon = DaemonControlAdapter::new(&pid_file);
    let dns = DnsAdapter::new(dns_service);
    let system = SystemSetupAdapter::new(paths);

    let use_case = Uninstall::new(
        &repo,
        &certs,
        &daemon,
        &dns,
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
