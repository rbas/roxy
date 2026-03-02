use crate::application::ports::{CertificateError, CertificateManager};
use crate::domain::DomainPattern;
use crate::infrastructure::certs::CertificateService;

/// Adapter that bridges [`CertificateService`] to the [`CertificateManager`] port.
pub struct CertificateAdapter<'a> {
    inner: &'a CertificateService,
}

impl<'a> CertificateAdapter<'a> {
    pub fn new(inner: &'a CertificateService) -> Self {
        Self { inner }
    }
}

impl CertificateManager for CertificateAdapter<'_> {
    fn init_ca(&self) -> Result<(), CertificateError> {
        self.inner
            .init_ca()
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn is_ca_installed(&self) -> Result<bool, CertificateError> {
        self.inner
            .is_ca_installed()
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn create_and_install(&self, pattern: &DomainPattern) -> Result<(), CertificateError> {
        self.inner
            .create_and_install(pattern)
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn remove(&self, pattern: &DomainPattern) -> Result<(), CertificateError> {
        self.inner
            .remove(pattern)
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn remove_ca(&self) -> Result<(), CertificateError> {
        self.inner
            .remove_ca()
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }

    fn exists(&self, pattern: &DomainPattern) -> bool {
        self.inner.exists(pattern)
    }

    fn is_trusted(&self) -> Result<bool, CertificateError> {
        self.inner
            .is_trusted()
            .map_err(|e| CertificateError::OperationFailed(e.into()))
    }
}
