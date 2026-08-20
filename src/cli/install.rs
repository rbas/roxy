use anyhow::Result;

use super::context::AppContext;
use crate::application::StepOutcome;
use crate::application::install::Install;
use crate::infrastructure::config::Config;
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::filesystem::FileSystemSetup;
use crate::infrastructure::network::get_network_info;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;
use crate::infrastructure::service;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub fn execute(
    ctx: &AppContext,
    config_path: &Path,
    paths: &RoxyPaths,
    config: &Config,
) -> Result<()> {
    service::validate_install_invocation(config_path, paths)?;
    println!("Setting up Roxy...\n");

    migrate_legacy_ca(paths)?;

    let dns_service = get_dns_service()?;
    let network_info = get_network_info();
    let system = FileSystemSetup::new(paths);

    let use_case = Install::new(
        &ctx.cert_service,
        &ctx.config_store,
        &dns_service,
        &network_info,
        &system,
        config.daemon.dns_port,
    );
    let result = use_case.execute()?;
    let stopped_legacy_daemon = stop_legacy_daemon()?;
    let runtime_user = service::install(config_path, paths, &config.daemon)?;

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
    if stopped_legacy_daemon {
        println!("  Migration: previous root daemon stopped and replaced.");
    }

    println!("\nRoxy installation complete!");
    println!("  Daemon user: {}", runtime_user.name);
    println!();
    println!("Register domains without sudo: roxy register <domain> --route \"/=3000\"");

    Ok(())
}

fn stop_legacy_daemon() -> Result<bool> {
    let pid_file = PidFile::new(PathBuf::from("/var/run/roxy.pid"));
    let Some(pid) = pid_file.get_running_pid()? else {
        return Ok(false);
    };

    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()?;
    let executable = String::from_utf8_lossy(&output.stdout);
    let is_roxy = Path::new(executable.trim())
        .file_name()
        .is_some_and(|name| name == "roxy");
    if !output.status.success() || !is_roxy {
        anyhow::bail!(
            "Legacy PID file points to a non-Roxy process ({pid}); remove \
             /var/run/roxy.pid after verifying that process"
        );
    }

    pid_file.stop_gracefully(Duration::from_secs(2))?;
    Ok(true)
}

fn migrate_legacy_ca(paths: &RoxyPaths) -> Result<()> {
    let legacy_dir = Path::new("/etc/roxy");
    let pairs = [
        (legacy_dir.join("ca.crt"), paths.data_dir.join("ca.crt")),
        (legacy_dir.join("ca.key"), paths.data_dir.join("ca.key")),
    ];
    if pairs.iter().all(|(_, destination)| destination.exists())
        || !pairs.iter().all(|(source, _)| source.exists())
    {
        return Ok(());
    }

    fs::create_dir_all(&paths.data_dir)?;
    for (source, destination) in pairs {
        fs::copy(source, destination)?;
    }
    crate::infrastructure::file_security::restrict_key_permissions(&paths.data_dir.join("ca.key"))?;
    Ok(())
}
