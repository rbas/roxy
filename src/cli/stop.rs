use anyhow::Result;

use crate::application::stop_daemon::StopDaemon;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let service = StopDaemon::new(&pid_file);
    service.execute()?;
    println!("Roxy daemon stopped.");
    Ok(())
}
