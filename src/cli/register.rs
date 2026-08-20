use anyhow::Result;

use super::context::AppContext;
use crate::application::register_domain::RegisterDomain;
use crate::domain::{DomainPattern, Route};

pub fn execute(
    domain: String,
    wildcard: bool,
    routes: Vec<String>,
    ctx: &AppContext,
) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let parsed_routes: Vec<Route> = routes
        .iter()
        .map(|s| Route::parse(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid route: {}", e))?;

    let use_case = RegisterDomain::new(&ctx.config_store);

    let result = use_case.execute(pattern, parsed_routes)?;
    ctx.reload_if_running()?;

    println!(
        "\nRegistered domain: {}",
        result.registration.display_pattern()
    );
    println!("  Routes:");
    for route in result.registration.routes() {
        println!("    {} -> {}", route.path(), route.target());
    }
    println!(
        "  HTTPS: {}",
        if result.registration.is_https_enabled() {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\nThe running proxy has been updated.");

    Ok(())
}
