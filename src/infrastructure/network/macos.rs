use std::net::Ipv4Addr;

use super::NetworkInfo;

pub struct MacOsNetworkInfo;

impl MacOsNetworkInfo {
    pub fn new() -> Self {
        Self
    }
}

impl NetworkInfo for MacOsNetworkInfo {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        // Try en0 first (usually WiFi on Mac)
        if let Some(ip) = get_ip_for_interface("en0") {
            return Some(ip);
        }

        // Try en1 (usually Ethernet on Mac)
        if let Some(ip) = get_ip_for_interface("en1") {
            return Some(ip);
        }

        // Try en2-en5 for other network interfaces
        for i in 2..=5 {
            if let Some(ip) = get_ip_for_interface(&format!("en{}", i)) {
                return Some(ip);
            }
        }

        None
    }
}

fn get_ip_for_interface(interface: &str) -> Option<Ipv4Addr> {
    let output = std::process::Command::new("ipconfig")
        .args(["getifaddr", interface])
        .output()
        .ok()?;

    if output.status.success() {
        let ip_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let ip: Ipv4Addr = ip_str.parse().ok()?;
        // Ensure it's a private IP, not link-local
        if ip.is_private() && !ip.is_link_local() {
            return Some(ip);
        }
    }

    None
}
