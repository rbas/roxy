use anyhow::Result;

use crate::application::stop_daemon::StopDaemon;
use crate::infrastructure::adapters::DaemonControlAdapter;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let daemon = DaemonControlAdapter::new(&pid_file);
    let service = StopDaemon::new(&daemon);
    service.execute()?;
    println!("Roxy daemon stopped.");
    Ok(())
}
