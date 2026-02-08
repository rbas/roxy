use std::net::Ipv4Addr;

/// Get the primary LAN IPv4 address of the host.
/// Returns 127.0.0.1 as fallback if no network is available.
pub fn get_lan_ip() -> Ipv4Addr {
    get_lan_ip_impl().unwrap_or(Ipv4Addr::new(127, 0, 0, 1))
}

#[cfg(target_os = "macos")]
fn get_lan_ip_impl() -> Option<Ipv4Addr> {
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

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "linux")]
fn get_lan_ip_impl() -> Option<Ipv4Addr> {
    // On Linux, use hostname -I which returns all IPs
    let output = std::process::Command::new("hostname")
        .arg("-I")
        .output()
        .ok()?;

    if output.status.success() {
        let output_str = String::from_utf8_lossy(&output.stdout);
        // hostname -I returns space-separated IPs, take the first private one
        for ip_str in output_str.split_whitespace() {
            if let Ok(ip) = ip_str.parse::<Ipv4Addr>() {
                if ip.is_private() && !ip.is_link_local() {
                    return Some(ip);
                }
            }
        }
    }

    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_lan_ip_impl() -> Option<Ipv4Addr> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_lan_ip_returns_valid_ip() {
        let ip = get_lan_ip();
        // Should be either a private IP or loopback
        assert!(ip.is_private() || ip.is_loopback());
    }

    #[test]
    fn test_get_lan_ip_not_link_local() {
        let ip = get_lan_ip();
        // Should not be link-local (169.254.x.x) unless it's loopback fallback
        assert!(ip.is_loopback() || !ip.is_link_local());
    }
}
