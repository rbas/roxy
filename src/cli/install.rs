use std::path::Path;

use anyhow::Result;

use crate::application::StepOutcome;
use crate::application::install::Install;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::{Config, ConfigStore};
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(config_path: &Path, paths: &RoxyPaths, config: &Config) -> Result<()> {
    println!("Setting up Roxy...\n");

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let use_case = Install::new(&config_store, &cert_service, config_path, paths, config);
    let result = use_case.execute()?;

    println!("  Using IP address: {}", result.lan_ip);
    if result.lan_ip.is_loopback() {
        println!("  Warning: No network detected, using localhost.");
    }

    for (label, outcome) in &result.steps {
        match outcome {
            StepOutcome::Success(msg) => println!("  {} {}", label, msg),
            StepOutcome::Warning(msg) => eprintln!("  {} {}", label, msg),
            StepOutcome::Skipped(msg) => println!("  {} {}", label, msg),
        }
    }

    println!("\nRoxy installation complete!");
    println!();
    println!("Register domains with: roxy register <domain> --port <port>");

    Ok(())
}
