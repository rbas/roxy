use std::net::Ipv4Addr;

use crate::application::ports::NetworkInfo as NetworkInfoPort;
use crate::infrastructure::network::{NetworkInfo as InfraNetworkInfo, PlatformNetworkInfo};

/// Adapter that bridges an infrastructure [`NetworkInfo`](InfraNetworkInfo)
/// to the application [`NetworkInfo`](NetworkInfoPort) port.
pub struct NetworkInfoAdapter {
    inner: PlatformNetworkInfo,
}

impl NetworkInfoAdapter {
    pub fn new(inner: PlatformNetworkInfo) -> Self {
        Self { inner }
    }
}

impl NetworkInfoPort for NetworkInfoAdapter {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        self.inner.lan_ip()
    }
}
