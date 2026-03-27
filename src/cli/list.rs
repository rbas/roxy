use anyhow::Result;

use super::context::AppContext;
use crate::application::list_all_domains::ListAllDomains;
use crate::domain::{RegistrationSource, RouteTarget};

pub fn execute(ctx: &AppContext) -> Result<()> {
    let docker_enabled = ctx
        .config_store
        .load()
        .map(|c| c.docker.enabled)
        .unwrap_or(false);

    let use_case = ListAllDomains::new(&ctx.mgmt_client, &ctx.config_store, &ctx.cert_service);
    let result = use_case.execute()?;

    if result.domains.is_empty() {
        println!("No domains registered.");
        println!("\nRegister a domain with:");
        println!("  roxy register myapp.roxy --route \"/=3000\"");
        println!("  roxy register myapp.roxy --route \"/=3000\" --route \"/api=3001\"");
        if !result.daemon_reachable && docker_enabled {
            println!("\n  Note: Docker domains are only visible when the daemon is running.");
        }
        return Ok(());
    }

    println!("Registered domains:\n");

    for info in result.domains {
        let https_status = if info.has_cert {
            match info.cert_trusted {
                Some(true) => "(HTTPS)",
                Some(false) => "(HTTPS untrusted)",
                None => "(HTTPS error)",
            }
        } else {
            ""
        };

        let source_label = if info.registration.source() == RegistrationSource::External {
            " [external]"
        } else {
            ""
        };

        println!(
            "  {}{} {}",
            info.registration.display_pattern(),
            source_label,
            https_status,
        );

        for route in info.registration.routes() {
            let target_str = match route.target() {
                RouteTarget::Proxy(p) => p.to_string(),
                RouteTarget::StaticFiles(p) => p.display().to_string(),
            };
            println!("    {:<15} -> {}", route.path(), target_str);
        }
        println!();
    }

    if !result.daemon_reachable && docker_enabled {
        println!("  Note: Docker domains are only visible when the daemon is running.");
    }

    Ok(())
}
