use anyhow::{Result, bail};

use crate::infrastructure::pid::PidFile;

pub fn execute() -> Result<()> {
    let pid_file = PidFile::new();

    if !pid_file.is_running()? {
        bail!("Roxy daemon is not running.\nStart it with: sudo roxy start");
    }

    println!("Reloading Roxy daemon...");

    // Stop the daemon
    super::stop::execute()?;

    // Brief pause to ensure clean shutdown
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Start the daemon
    super::start::execute(false)?;

    println!("Daemon reloaded with updated configuration.");
    Ok(())
}
