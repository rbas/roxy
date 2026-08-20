use anyhow::Result;

use super::context::AppContext;
use crate::application::daemon_status::QueryDaemonStatus;
use crate::application::list_all_domains::ListAllDomains;
use crate::config::DaemonConfig;
use crate::domain::RegistrationSource;
use crate::infrastructure::network::get_network_info;

pub fn execute(ctx: &AppContext, daemon_config: &DaemonConfig) -> Result<()> {
    let network_info = get_network_info();

    let service = QueryDaemonStatus::new(&ctx.pid_file, &ctx.cert_service, &network_info);
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
            println!("  HTTP:  http://localhost:{}", daemon_config.http_port);
            println!("  HTTPS: https://localhost:{}", daemon_config.https_port);
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
            println!("\nStart with: roxy start");
        }
    }

    // List domains via ListAllDomains (daemon + Docker, or config fallback)
    let list_svc = ListAllDomains::new(&ctx.mgmt_client, &ctx.config_store, &ctx.cert_service);
    let result = list_svc.execute()?;

    if !result.domains.is_empty() {
        println!("\nRegistered domains: {}", result.domains.len());
        for info in result.domains {
            let scheme = if info.registration.is_https_enabled() {
                "https"
            } else {
                "http"
            };
            let source_label = if info.registration.source() == RegistrationSource::External {
                " [external]"
            } else {
                ""
            };
            println!(
                "  {}://{}{}",
                scheme,
                info.registration.display_pattern(),
                source_label,
            );
        }
    }

    Ok(())
}
