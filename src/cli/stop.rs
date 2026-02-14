use std::time::Duration;

use anyhow::{Result, bail};

use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());

    if pid_file.get_running_pid()?.is_none() {
        bail!("Roxy daemon is not running.");
    }

    pid_file.stop_gracefully(Duration::from_millis(500))?;
    println!("Roxy daemon stopped.");

    Ok(())
}
