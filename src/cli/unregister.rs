use anyhow::Result;

use super::context::AppContext;
use crate::application::unregister_domain::UnregisterDomain;
use crate::domain::DomainPattern;

pub fn execute(domain: String, wildcard: bool, force: bool, ctx: &AppContext) -> Result<()> {
    let pattern = DomainPattern::from_name(&domain, wildcard)?;

    let use_case = UnregisterDomain::new(&ctx.config_store);

    if !force {
        let registration = use_case.preview(&pattern)?;
        println!("This will unregister the domain:");
        println!("  Domain: {}", registration.display_pattern());
        println!("  Routes:");
        for route in registration.routes() {
            println!("    {} -> {}", route.path(), route.target());
        }
        println!("\nRun with --force to confirm.");
        return Ok(());
    }

    let result = use_case.execute(&pattern)?;
    ctx.reload_if_running()?;

    println!(
        "Unregistered domain: {}",
        result.registration.display_pattern()
    );

    Ok(())
}
