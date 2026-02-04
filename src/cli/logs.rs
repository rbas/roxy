use anyhow::Result;

use crate::infrastructure::logging::LogFile;

pub fn execute(lines: usize, clear: bool) -> Result<()> {
    let log_file = LogFile::new();

    if clear {
        log_file.clear()?;
        println!("Logs cleared.");
        return Ok(());
    }

    let content = log_file.tail(lines)?;

    if content.is_empty() {
        println!("No logs found.");
        println!("Log file: {}", log_file.path().display());
        println!("\nStart the daemon to generate logs: sudo roxy start");
        return Ok(());
    }

    println!("{}", content);
    Ok(())
}
