use std::path::Path;

use anyhow::{Result, bail};

use crate::domain::DomainName;
use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;

pub fn execute(
    domain: String,
    wildcard: bool,
    force: bool,
    config_path: &Path,
    paths: &RoxyPaths,
) -> Result<()> {
    let domain = DomainName::new(&domain)?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let cert_service = CertificateService::new(paths);

    // Check if domain exists
    let registration = if wildcard {
        config_store.get_wildcard_domain(&domain)?
    } else {
        config_store.get_domain(&domain)?
    };
    if registration.is_none() {
        bail!(
            "Domain '{}' is not registered.",
            if wildcard {
                format!("*.{}", domain.as_str())
            } else {
                domain.as_str().to_string()
            }
        );
    }

    let registration = registration.unwrap();

    if !force {
        println!("This will unregister the domain:");
        println!("  Domain: {}", registration.display_pattern());
        println!("  Routes:");
        for route in &registration.routes {
            println!("    {} -> {}", route.path, route.target);
        }
        if registration.https_enabled {
            println!("  HTTPS certificate files will be removed");
        }
        println!("\nRun with --force to confirm.");
        return Ok(());
    }

    // Remove certificate if exists
    let cert_exists = if wildcard {
        cert_service.exists_wildcard(&domain)
    } else {
        cert_service.exists(&domain)
    };
    if cert_exists {
        println!("Removing SSL certificate...");
        let removal = if wildcard {
            cert_service.remove_wildcard(&domain)
        } else {
            cert_service.remove(&domain)
        };
        match removal {
            Ok(()) => println!("  Certificate removed."),
            Err(e) => {
                eprintln!("  Warning: Failed to remove certificate: {}", e);
            }
        }
    }

    // Remove from config
    if wildcard {
        config_store.remove_wildcard_domain(&domain)?;
    } else {
        config_store.remove_domain(&domain)?;
    }

    println!("Unregistered domain: {}", registration.display_pattern());

    Ok(())
}
