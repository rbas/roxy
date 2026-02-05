use anyhow::Result;

use super::{start, stop};
use crate::infrastructure::pid::PidFile;

pub fn execute() -> Result<()> {
    let pid_file = PidFile::new();

    if pid_file.is_running()? {
        println!("Stopping Roxy daemon...");
        stop::execute()?;
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    println!("Starting Roxy daemon...");
    start::execute(false, false)
}
