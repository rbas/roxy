use std::path::Path;

use anyhow::Result;

use super::context::AppContext;
use crate::application::restart_daemon::RestartDaemon;

pub fn execute(verbose: bool, config_path: &Path, ctx: &AppContext) -> Result<()> {
    let service = RestartDaemon::new(&ctx.pid_file, &ctx.config_store);
    let ready = service.reload()?;

    println!("Starting Roxy daemon...");
    super::start::execute(
        false,
        verbose,
        config_path,
        &ready.paths,
        &ready.daemon_config,
    )?;

    println!("Daemon reloaded with updated configuration.");
    Ok(())
}
