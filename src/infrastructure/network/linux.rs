use std::net::Ipv4Addr;

use crate::application::ports::NetworkInfo;

pub struct LinuxNetworkInfo;

impl LinuxNetworkInfo {
    pub fn new() -> Self {
        Self
    }
}

impl NetworkInfo for LinuxNetworkInfo {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        // On Linux, use hostname -I which returns all IPs
        let output = std::process::Command::new("hostname")
            .arg("-I")
            .output()
            .ok()?;

        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // hostname -I returns space-separated IPs, take the first private one
            for ip_str in output_str.split_whitespace() {
                if let Ok(ip) = ip_str.parse::<Ipv4Addr>()
                    && ip.is_private()
                    && !ip.is_link_local()
                {
                    return Some(ip);
                }
            }
        }

        None
    }
}
