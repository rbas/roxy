#[derive(Debug, thiserror::Error)]
pub enum CertificateError {
    #[error("{0}")]
    OperationFailed(#[source] anyhow::Error),
}

/// Port for certificate lifecycle operations.
pub trait CertificateManager {
    /// Initialize the Root CA and install it in the system trust store.
    fn init_ca(&self) -> Result<(), CertificateError>;

    /// Check if the Root CA exists and is trusted.
    fn is_ca_installed(&self) -> Result<bool, CertificateError>;

    /// Remove the Root CA from the trust store and delete CA files.
    fn remove_ca(&self) -> Result<(), CertificateError>;

    /// Check if the CA certificate is trusted by the system.
    fn is_trusted(&self) -> Result<bool, CertificateError>;
}
