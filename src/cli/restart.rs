use std::path::Path;

use anyhow::Result;

use crate::application::restart_daemon::RestartDaemon;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(verbose: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let service = RestartDaemon::new(&pid_file, &config_store);
    let ready = service.execute(false)?;

    println!("Starting Roxy daemon...");
    super::start::execute(
        false,
        verbose,
        config_path,
        &ready.paths,
        &ready.daemon_config,
    )
}
