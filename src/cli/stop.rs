use anyhow::{Result, bail};

use crate::infrastructure::pid::PidFile;

pub fn execute() -> Result<()> {
    let pid_file = PidFile::new();

    let pid = match pid_file.get_running_pid()? {
        Some(pid) => pid,
        None => bail!("Roxy daemon is not running."),
    };

    // Send SIGTERM to the process
    #[cfg(unix)]
    {
        use std::process::Command;
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output()?;
    }

    // Wait a moment and check if it stopped
    std::thread::sleep(std::time::Duration::from_millis(500));

    if pid_file.is_running()? {
        // Force kill if still running
        #[cfg(unix)]
        {
            use std::process::Command;
            Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output()?;
        }
    }

    pid_file.remove()?;
    println!("Roxy daemon stopped.");

    Ok(())
}
