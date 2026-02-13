use std::path::Path;

use anyhow::Result;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);

    // Get LAN IP
    let lan_ip = get_lan_ip();
    let offline_note = if lan_ip.is_loopback() {
        " (offline)"
    } else {
        ""
    };

    // Check CA status
    let ca_installed = cert_service.is_ca_installed().unwrap_or(false);

    // Check daemon status
    match pid_file.get_running_pid()? {
        Some(pid) => {
            println!("Roxy daemon: running (PID: {})", pid);
            println!("  LAN IP: {}{}", lan_ip, offline_note);
            println!(
                "  Root CA: {}",
                if ca_installed {
                    "installed"
                } else {
                    "not installed"
                }
            );
            println!("  HTTP:  http://localhost:80");
            println!("  HTTPS: https://localhost:443");
            if !lan_ip.is_loopback() {
                println!("\n  Access from other devices: use http://{}", lan_ip);
            }
        }
        None => {
            println!("Roxy daemon: stopped");
            println!("  LAN IP: {}{}", lan_ip, offline_note);
            println!(
                "  Root CA: {}",
                if ca_installed {
                    "installed"
                } else {
                    "not installed"
                }
            );
            println!("\nStart with: sudo roxy start");
        }
    }

    // Show registered domains
    let domains = config_store.list_domains()?;
    if !domains.is_empty() {
        println!("\nRegistered domains: {}", domains.len());
        for domain in domains {
            let scheme = if domain.https_enabled {
                "https"
            } else {
                "http"
            };
            if domain.wildcard {
                println!("  {}://{} (wildcard)", scheme, domain.domain);
            } else {
                println!("  {}://{}", scheme, domain.domain);
            }
        }
    }

    Ok(())
}
