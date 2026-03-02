use crate::application::ports::{DnsConfigError, DnsManager};
use crate::infrastructure::dns::{DnsError, DnsService};

/// Adapter that bridges a [`DnsService`] to the [`DnsManager`] port.
pub struct DnsAdapter {
    inner: Box<dyn DnsService>,
}

impl DnsAdapter {
    pub fn new(inner: Box<dyn DnsService>) -> Self {
        Self { inner }
    }
}

impl DnsManager for DnsAdapter {
    fn setup(&self, port: u16) -> Result<(), DnsConfigError> {
        self.inner.setup(port).map_err(map_dns_error)
    }

    fn cleanup(&self) -> Result<(), DnsConfigError> {
        self.inner.cleanup().map_err(map_dns_error)
    }

    fn validate(&self) -> Result<(), DnsConfigError> {
        self.inner.validate().map_err(map_dns_error)
    }

    fn is_configured(&self) -> bool {
        self.inner.is_configured()
    }
}

fn map_dns_error(e: DnsError) -> DnsConfigError {
    match e {
        DnsError::PermissionDenied => DnsConfigError::PermissionDenied,
        DnsError::ValidationFailed(m) => DnsConfigError::ValidationFailed(m),
        DnsError::UnsupportedPlatform(p) => DnsConfigError::UnsupportedPlatform(p),
        other => DnsConfigError::OperationFailed(other.into()),
    }
}
