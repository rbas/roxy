use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::ProcessControl;

pub struct UnixProcessControl;

impl ProcessControl for UnixProcessControl {
    fn process_exists(&self, pid: u32) -> bool {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn terminate(&self, pid: u32, timeout: Duration) -> Result<()> {
        let output = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to send SIGTERM to pid {pid}: {stderr}");
        }

        let deadline = Instant::now() + timeout;
        while self.process_exists(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }

        if self.process_exists(pid) {
            let output = Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to send SIGKILL to pid {pid}: {stderr}");
            }
        }

        Ok(())
    }
}
