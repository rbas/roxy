use std::path::Path;

use anyhow::Result;

use crate::application::manage_routes::ManageRoutes;
use crate::domain::{DomainPattern, PathPrefix, RouteTarget};
use crate::infrastructure::config::ConfigStore;

/// Add a route to an existing domain
pub fn add(
    domain: String,
    wildcard: bool,
    path: String,
    target: String,
    config_path: &Path,
) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;
    let path_prefix = PathPrefix::new(&path)?;
    let route_target = RouteTarget::parse(&target)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target, e))?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let use_case = ManageRoutes::new(&config_store);

    let route = use_case.add_route(&pattern, path_prefix, route_target)?;

    println!("Added route: {} -> {}", route.path, route.target);
    println!("\nReload the daemon to apply changes: roxy reload");

    Ok(())
}

/// Remove a route from a domain
pub fn remove(domain: String, wildcard: bool, path: String, config_path: &Path) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;
    let path_prefix = PathPrefix::new(&path)?;

    let config_store = ConfigStore::new(config_path.to_path_buf());
    let use_case = ManageRoutes::new(&config_store);

    use_case.remove_route(&pattern, &path_prefix)?;

    println!("Removed route: {}", path_prefix);
    println!("\nReload the daemon to apply changes: roxy reload");

    Ok(())
}

/// List all routes for a domain
pub fn list(domain: String, wildcard: bool, config_path: &Path) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let config_store = ConfigStore::new(config_path.to_path_buf());

    let registration = config_store
        .get_domain(&pattern)?
        .ok_or_else(|| anyhow::anyhow!("Domain '{}' not registered", pattern))?;

    if registration.routes().is_empty() {
        println!(
            "No routes configured for {}",
            registration.display_pattern()
        );
        return Ok(());
    }

    println!("Routes for {}:\n", registration.display_pattern());
    println!("{:<20} {:<30}", "PATH", "TARGET");
    println!("{}", "-".repeat(52));

    for route in registration.routes() {
        println!("{:<20} {:<30}", route.path, route.target);
    }

    Ok(())
}
