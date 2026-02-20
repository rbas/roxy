use std::fs;
use std::path::Path;
use std::process::Command;

use super::super::CertError;
use super::TrustStore;

const SYSTEM_CERT_PATH: &str = "/usr/local/share/ca-certificates/roxy-ca.crt";

/// Linux trust store implementation using `update-ca-certificates`.
///
/// Standard on Debian/Ubuntu systems. Installs the Roxy Root CA into
/// the system certificate store so all domain certificates signed by
/// it are automatically trusted.
pub struct LinuxTrustStore;

impl LinuxTrustStore {
    pub fn new() -> Self {
        Self
    }
}

impl TrustStore for LinuxTrustStore {
    fn add_ca(&self, cert_path: &Path) -> Result<(), CertError> {
        // Copy the CA certificate to the system certificates directory
        fs::copy(cert_path, SYSTEM_CERT_PATH).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                CertError::PermissionDenied
            } else {
                CertError::TrustStoreError(format!(
                    "Failed to copy CA certificate to {}: {}",
                    SYSTEM_CERT_PATH, e
                ))
            }
        })?;

        // Update the system trust store
        let output = Command::new("update-ca-certificates")
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to run update-ca-certificates: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            if stderr.contains("Permission denied") {
                return Err(CertError::PermissionDenied);
            }

            return Err(CertError::TrustStoreError(format!(
                "update-ca-certificates failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn remove_ca(&self) -> Result<(), CertError> {
        let path = Path::new(SYSTEM_CERT_PATH);
        if !path.exists() {
            return Ok(());
        }

        fs::remove_file(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                CertError::PermissionDenied
            } else {
                CertError::TrustStoreError(format!(
                    "Failed to remove CA certificate from {}: {}",
                    SYSTEM_CERT_PATH, e
                ))
            }
        })?;

        // Rebuild the trust store without the removed certificate
        let output = Command::new("update-ca-certificates")
            .arg("--fresh")
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!(
                    "Failed to run update-ca-certificates --fresh: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CertError::TrustStoreError(format!(
                "update-ca-certificates --fresh failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn is_ca_trusted(&self) -> Result<bool, CertError> {
        Ok(Path::new(SYSTEM_CERT_PATH).exists())
    }
}

impl Default for LinuxTrustStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_store_creation() {
        let _store = LinuxTrustStore::new();
    }
}
