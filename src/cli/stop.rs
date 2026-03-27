use anyhow::Result;

use super::context::AppContext;
use crate::application::stop_daemon::StopDaemon;

pub fn execute(ctx: &AppContext) -> Result<()> {
    let service = StopDaemon::new(&ctx.pid_file);
    service.execute()?;
    println!("Roxy daemon stopped.");
    Ok(())
}
