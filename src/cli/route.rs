use anyhow::Result;

use crate::domain::{DomainName, PathPrefix, Route, RouteTarget};
use crate::infrastructure::config::ConfigStore;

/// Add a route to an existing domain
pub fn add(domain: String, path: String, target: String) -> Result<()> {
    let domain = DomainName::new(&domain)?;
    let path_prefix = PathPrefix::new(&path)?;
    let route_target = RouteTarget::parse(&target)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target, e))?;

    let config_store = ConfigStore::new();

    // Get existing registration
    let mut registration = config_store
        .get_domain(&domain)?
        .ok_or_else(|| anyhow::anyhow!("Domain '{}' not registered", domain))?;

    // Add the route
    let route = Route::new(path_prefix.clone(), route_target.clone());
    registration.add_route(route)?;

    // Save updated registration
    config_store.update_domain(registration)?;

    println!("Added route: {} -> {}", path_prefix, route_target);
    println!("\nReload the daemon to apply changes: roxy reload");

    Ok(())
}

/// Remove a route from a domain
pub fn remove(domain: String, path: String) -> Result<()> {
    let domain = DomainName::new(&domain)?;
    let path_prefix = PathPrefix::new(&path)?;

    let config_store = ConfigStore::new();

    // Get existing registration
    let mut registration = config_store
        .get_domain(&domain)?
        .ok_or_else(|| anyhow::anyhow!("Domain '{}' not registered", domain))?;

    // Remove the route
    registration.remove_route(&path_prefix)?;

    // Save updated registration
    config_store.update_domain(registration)?;

    println!("Removed route: {}", path_prefix);
    println!("\nReload the daemon to apply changes: roxy reload");

    Ok(())
}

/// List all routes for a domain
pub fn list(domain: String) -> Result<()> {
    let domain = DomainName::new(&domain)?;

    let config_store = ConfigStore::new();

    // Get existing registration
    let registration = config_store
        .get_domain(&domain)?
        .ok_or_else(|| anyhow::anyhow!("Domain '{}' not registered", domain))?;

    if registration.routes.is_empty() {
        println!("No routes configured for {}", domain);
        return Ok(());
    }

    println!("Routes for {}:\n", domain);
    println!("{:<20} {:<30}", "PATH", "TARGET");
    println!("{}", "-".repeat(52));

    for route in &registration.routes {
        println!("{:<20} {:<30}", route.path, route.target);
    }

    Ok(())
}
