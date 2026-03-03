use std::path::Path;

use anyhow::Result;

use crate::application::daemon_status::QueryDaemonStatus;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::network::get_network_info;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);
    let network_info = get_network_info();

    let service = QueryDaemonStatus::new(&pid_file, &config_store, &cert_service, &network_info);
    let status = service.execute()?;

    let offline_note = if status.lan_ip.is_loopback() {
        " (offline)"
    } else {
        ""
    };

    let ca_label = if status.ca_installed {
        "installed"
    } else {
        "not installed"
    };

    match status.pid {
        Some(pid) => {
            println!("Roxy daemon: running (PID: {})", pid);
            println!("  LAN IP: {}{}", status.lan_ip, offline_note);
            println!("  Root CA: {}", ca_label);
            println!("  HTTP:  http://localhost:80");
            println!("  HTTPS: https://localhost:443");
            if !status.lan_ip.is_loopback() {
                println!(
                    "\n  Access from other devices: use http://{}",
                    status.lan_ip
                );
            }
        }
        None => {
            println!("Roxy daemon: stopped");
            println!("  LAN IP: {}{}", status.lan_ip, offline_note);
            println!("  Root CA: {}", ca_label);
            println!("\nStart with: sudo roxy start");
        }
    }

    if !status.domains.is_empty() {
        println!("\nRegistered domains: {}", status.domains.len());
        for reg in status.domains {
            let scheme = if reg.is_https_enabled() {
                "https"
            } else {
                "http"
            };
            println!("  {}://{}", scheme, reg.display_pattern());
        }
    }

    Ok(())
}
