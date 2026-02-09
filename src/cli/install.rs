use std::path::Path;

use anyhow::Result;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::{Config, ConfigStore};
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(config_path: &Path, paths: &RoxyPaths, config: &Config) -> Result<()> {
    println!("Setting up Roxy...\n");

    let dns_port = config.daemon.dns_port;

    // Detect LAN IP
    let lan_ip = get_lan_ip();
    println!("  Using IP address: {}", lan_ip);
    if lan_ip.is_loopback() {
        println!("  Warning: No network detected, using localhost.");
    }

    // Ensure data directory exists
    std::fs::create_dir_all(&paths.data_dir)?;
    std::fs::create_dir_all(&paths.certs_dir)?;

    // Ensure log directory exists
    if let Some(log_dir) = paths.log_file.parent() {
        std::fs::create_dir_all(log_dir)?;
    }

    // Write default config if it doesn't exist
    if !config_path.exists() {
        let config_store = ConfigStore::new(config_path.to_path_buf());
        config_store.save(config)?;
        println!("  Created config file: {}", config_path.display());
    }

    // Step 1: Initialize Root CA
    let cert_service = CertificateService::new(paths);
    match cert_service.is_ca_installed() {
        Ok(true) => {
            println!("  Root CA already installed, skipping...");
        }
        _ => {
            println!("  Creating Root CA for SSL certificates...");
            match cert_service.init_ca() {
                Ok(()) => {
                    println!("  Root CA created and installed in system trust store.");
                }
                Err(e) => {
                    eprintln!("  Warning: Failed to create Root CA: {}", e);
                    eprintln!("  HTTPS certificates will not work.");
                    eprintln!("  Run 'sudo roxy install' to enable HTTPS.");
                }
            }
        }
    }

    // Step 2: Configure DNS
    let dns = get_dns_service()?;
    if dns.is_configured() {
        println!("  DNS already configured, skipping...");
    } else {
        println!(
            "  Configuring DNS for *.roxy domains (port {})...",
            dns_port
        );
        dns.setup(dns_port)?;
        println!("  DNS configured successfully.");
    }

    // Step 3: Validate DNS
    println!("  Validating DNS configuration...");
    dns.validate()?;
    println!("  DNS validation passed.\n");

    println!("Roxy installation complete!");
    println!();
    println!("Register domains with: roxy register <domain> --port <port>");

    Ok(())
}
