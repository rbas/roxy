use std::net::Ipv4Addr;

use crate::application::ports::NetworkInfo as NetworkInfoPort;
use crate::infrastructure::network::NetworkInfo as InfraNetworkInfo;

/// Adapter that bridges an infrastructure [`NetworkInfo`](InfraNetworkInfo)
/// to the application [`NetworkInfo`](NetworkInfoPort) port.
pub struct NetworkInfoAdapter {
    inner: Box<dyn InfraNetworkInfo>,
}

impl NetworkInfoAdapter {
    pub fn new(inner: Box<dyn InfraNetworkInfo>) -> Self {
        Self { inner }
    }
}

impl NetworkInfoPort for NetworkInfoAdapter {
    fn lan_ip(&self) -> Option<Ipv4Addr> {
        self.inner.lan_ip()
    }
}
