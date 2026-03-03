use std::net::Ipv4Addr;

/// Port for querying host network information.
pub trait NetworkInfo {
    /// Get the primary LAN IPv4 address, if available.
    fn lan_ip(&self) -> Option<Ipv4Addr>;
}
