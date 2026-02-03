use anyhow::Result;

use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::pid::PidFile;

pub fn execute() -> Result<()> {
    let pid_file = PidFile::new();
    let config_store = ConfigStore::new();

    // Check daemon status
    match pid_file.get_running_pid()? {
        Some(pid) => {
            println!("Roxy daemon: running (PID: {})", pid);
            println!("  HTTP:  http://localhost:80");
            println!("  HTTPS: https://localhost:443");
        }
        None => {
            println!("Roxy daemon: stopped");
            println!("\nStart with: sudo roxy start");
        }
    }

    // Show registered domains
    let domains = config_store.list_domains()?;
    if !domains.is_empty() {
        println!("\nRegistered domains: {}", domains.len());
        for domain in domains {
            let scheme = if domain.https_enabled {
                "https"
            } else {
                "http"
            };
            println!("  {}://{}", scheme, domain.domain);
        }
    }

    Ok(())
}
