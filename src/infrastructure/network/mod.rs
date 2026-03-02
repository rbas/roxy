use std::net::Ipv4Addr;

/// Platform-agnostic network information provider.
pub trait NetworkInfo {
    /// Get the primary LAN IPv4 address, if available.
    fn lan_ip(&self) -> Option<Ipv4Addr>;
}

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

/// Get the network info provider for the current platform.
#[cfg(target_os = "macos")]
pub fn get_network_info() -> Box<dyn NetworkInfo> {
    Box::new(macos::MacOsNetworkInfo::new())
}

/// Get the network info provider for the current platform.
#[cfg(target_os = "linux")]
pub fn get_network_info() -> Box<dyn NetworkInfo> {
    Box::new(linux::LinuxNetworkInfo::new())
}

/// Get the network info provider for the current platform.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn get_network_info() -> Box<dyn NetworkInfo> {
    Box::new(FallbackNetworkInfo)
}

/// Get the primary LAN IPv4 address of the host.
/// Returns 127.0.0.1 as fallback if no network is available.
pub fn get_lan_ip() -> Ipv4Addr {
    get_network_info()
        .lan_ip()
        .unwrap_or(Ipv4Addr::new(127, 0, 0, 1))
}

/// Fallback for unsupported platforms — always returns None.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
struct FallbackNetworkInfo;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl NetworkInfo for FallbackNetworkInfo {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        None
    }
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

    #[test]
    fn test_get_network_info_returns_provider() {
        let info = get_network_info();
        // Should return either a valid IP or None (fallback returns None)
        if let Some(ip) = info.lan_ip() {
            assert!(ip.is_private() || ip.is_loopback());
        }
    }
}
