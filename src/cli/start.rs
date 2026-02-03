use anyhow::{bail, Result};
use std::env;
use std::process::{Command, Stdio};

use crate::infrastructure::pid::PidFile;

pub fn execute(foreground: bool) -> Result<()> {
    let pid_file = PidFile::new();

    // Check if already running
    if let Some(pid) = pid_file.get_running_pid()? {
        bail!(
            "Roxy daemon is already running (PID: {})\nUse 'roxy stop' to stop it first.",
            pid
        );
    }

    if foreground {
        // Run in foreground (blocking)
        run_server()
    } else {
        // Fork to background
        let exe = env::current_exe()?;

        let child = Command::new(exe)
            .args(["start", "--foreground"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        println!("Roxy daemon started (PID: {})", child.id());
        println!("Listening on http://localhost:80 and https://localhost:443");
        println!("\nUse 'roxy status' to check status");
        println!("Use 'roxy stop' to stop the daemon");

        Ok(())
    }
}

#[tokio::main]
async fn run_server() -> Result<()> {
    use crate::daemon::Server;
    use crate::infrastructure::pid::PidFile;

    let pid_file = PidFile::new();
    pid_file.write()?;

    // Handle Ctrl+C gracefully
    let pid_file_cleanup = PidFile::new();
    ctrlc::set_handler(move || {
        let _ = pid_file_cleanup.remove();
        std::process::exit(0);
    })?;

    println!("Starting Roxy daemon...");

    let server = Server::new()?;
    let result = server.run().await;

    pid_file.remove()?;
    result
}
