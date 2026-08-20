use anyhow::Result;

use super::context::AppContext;
use crate::application::manage_routes::ManageRoutes;
use crate::domain::{DomainPattern, PathPrefix, RouteTarget};

/// Add a route to an existing domain
pub fn add(
    domain: String,
    wildcard: bool,
    path: String,
    target: String,
    ctx: &AppContext,
) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;
    let path_prefix = PathPrefix::new(&path)?;
    let route_target = RouteTarget::parse(&target)
        .map_err(|e| anyhow::anyhow!("Invalid target '{}': {}", target, e))?;

    let use_case = ManageRoutes::new(&ctx.config_store);

    let route = use_case.add_route(&pattern, path_prefix, route_target)?;
    ctx.reload_if_running()?;

    println!("Added route: {} -> {}", route.path(), route.target());

    Ok(())
}

/// Remove a route from a domain
pub fn remove(domain: String, wildcard: bool, path: String, ctx: &AppContext) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;
    let path_prefix = PathPrefix::new(&path)?;

    let use_case = ManageRoutes::new(&ctx.config_store);

    use_case.remove_route(&pattern, &path_prefix)?;
    ctx.reload_if_running()?;

    println!("Removed route: {}", path_prefix);

    Ok(())
}

/// List all routes for a domain
pub fn list(domain: String, wildcard: bool, ctx: &AppContext) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let use_case = ManageRoutes::new(&ctx.config_store);

    let registration = use_case.list_routes(&pattern)?;

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
        println!("{:<20} {:<30}", route.path(), route.target());
    }

    Ok(())
}
