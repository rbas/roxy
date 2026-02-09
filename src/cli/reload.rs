use std::path::Path;

use anyhow::{Result, bail};

use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

pub fn execute(verbose: bool, config_path: &Path, paths: &RoxyPaths) -> Result<()> {
    let pid_file = PidFile::new(paths.pid_file.clone());

    if !pid_file.is_running()? {
        bail!("Roxy daemon is not running.\nStart it with: sudo roxy start");
    }

    println!("Reloading Roxy daemon...");

    // Stop the daemon
    super::stop::execute(paths)?;

    // Brief pause to ensure clean shutdown
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Re-load config from disk to pick up changes
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let fresh_config = config_store.load()?;

    // Start the daemon with fresh config
    super::start::execute(false, verbose, config_path, paths, &fresh_config)?;

    println!("Daemon reloaded with updated configuration.");
    Ok(())
}
