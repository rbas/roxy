use std::process::Command;
use std::time::Duration;

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
        Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output()?;

        std::thread::sleep(timeout);

        if self.process_exists(pid) {
            Command::new("kill")
                .args(["-KILL", &pid.to_string()])
                .output()?;
        }

        Ok(())
    }
}
