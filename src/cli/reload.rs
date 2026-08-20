use anyhow::Result;

use super::context::AppContext;
use crate::application::ports::DaemonConnection;

pub fn execute(ctx: &AppContext) -> Result<()> {
    ctx.mgmt_client.reload()?;
    println!("Daemon reloaded with updated configuration.");
    Ok(())
}
