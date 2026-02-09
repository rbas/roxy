use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::Result;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(force: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    if !force {
        println!("This will remove all Roxy configuration including:");
        println!("  - Stop the running daemon");
        println!("  - DNS configuration for *.roxy domains");
        println!("  - All registered domains");
        println!("  - All SSL certificates from system trust store");
        println!("  - All data in {}/", paths.data_dir.display());
        println!("\nRun with --force to confirm, or press Ctrl+C to cancel.");
        return Ok(());
    }

    println!("Uninstalling Roxy...\n");

    // Step 1: Stop daemon if running
    let pid_file = PidFile::new(paths.pid_file.clone());
    if let Some(pid) = pid_file.get_running_pid()? {
        println!("  Stopping daemon (PID: {})...", pid);
        stop_daemon(pid)?;
        pid_file.remove()?;
        println!("  Daemon stopped.");
    }

    // Step 2: Remove certificates from trust store
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let domains = config_store.list_domains().unwrap_or_default();

    let cert_service = CertificateService::new(paths);

    if !domains.is_empty() {
        println!("  Removing {} domain certificate(s)...", domains.len());

        for registration in &domains {
            match cert_service.remove(&registration.domain) {
                Ok(_) => println!("    - {} removed", registration.domain),
                Err(e) => println!("    - {} failed: {}", registration.domain, e),
            }
        }
    }

    // Remove Root CA from system trust store
    println!("  Removing Root CA from system trust store...");
    match cert_service.remove_ca() {
        Ok(_) => println!("  Root CA removed."),
        Err(e) => println!("  Failed to remove Root CA: {}", e),
    }

    // Step 3: Remove DNS configuration
    let dns = get_dns_service()?;
    if dns.is_configured() {
        println!("  Removing DNS configuration...");
        dns.cleanup()?;
        println!("  DNS configuration removed.");
    }

    // Step 4: Remove data directory entirely
    if paths.data_dir.exists() {
        println!("  Removing {}...", paths.data_dir.display());
        fs::remove_dir_all(&paths.data_dir)?;
        println!("  Directory removed.");
    }

    // Step 5: Remove PID file
    if paths.pid_file.exists() {
        let _ = fs::remove_file(&paths.pid_file);
    }

    // Step 6: Remove log directory
    if let Some(log_dir) = paths.log_file.parent()
        && log_dir.exists()
    {
        println!("  Removing {}...", log_dir.display());
        let _ = fs::remove_dir_all(log_dir);
        println!("  Log directory removed.");
    }

    println!("\nRoxy uninstallation complete!");
    println!("All configuration and certificates have been removed.");

    Ok(())
}

fn stop_daemon(pid: u32) -> Result<()> {
    // Send SIGTERM
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()?;

    // Wait briefly for graceful shutdown
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Check if still running, force kill if needed
    let status = Command::new("kill").args(["-0", &pid.to_string()]).output();

    if status.is_ok() && status.unwrap().status.success() {
        // Still running, force kill
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .output()?;
    }

    Ok(())
}
