use anyhow::Result;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::pid::PidFile;

pub fn execute() -> Result<()> {
    let pid_file = PidFile::new();
    let config_store = ConfigStore::new();
    let cert_service = CertificateService::new();

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
            println!("  Root CA: {}", if ca_installed { "installed" } else { "not installed" });
            println!("  HTTP:  http://localhost:80");
            println!("  HTTPS: https://localhost:443");
            if !lan_ip.is_loopback() {
                println!("\n  Access from Docker containers: use https://yourdomain.roxy");
                println!("  Access from other devices: use http://{}", lan_ip);
            }
        }
        None => {
            println!("Roxy daemon: stopped");
            println!("  LAN IP: {}{}", lan_ip, offline_note);
            println!("  Root CA: {}", if ca_installed { "installed" } else { "not installed" });
            println!("\nStart with: sudo roxy start");
        }
    }

    // Show CA mount info for Docker users
    if ca_installed {
        println!("\nDocker CA mount:");
        println!("  -v {}:/usr/local/share/ca-certificates/roxy.crt",
                 cert_service.ca_cert_path().display());
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
            println!("  {}://{}", scheme, domain.domain);
        }
    }

    Ok(())
}
