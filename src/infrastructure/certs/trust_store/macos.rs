use std::path::Path;
use std::process::Command;

use super::super::CertError;
use super::TrustStore;

const ROXY_CA_NAME: &str = "Roxy Local Development CA";

/// macOS Keychain trust store implementation
pub struct MacOsTrustStore;

impl MacOsTrustStore {
    pub fn new() -> Self {
        Self
    }
}

impl TrustStore for MacOsTrustStore {
    fn add_ca(&self, cert_path: &Path) -> Result<(), CertError> {
        // Add CA certificate to system keychain as a trusted root
        let output = Command::new("security")
            .args([
                "add-trusted-cert",
                "-d",
                "-r",
                "trustRoot",
                "-k",
                "/Library/Keychains/System.keychain",
                cert_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to run security command: {}", e))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            if stderr.contains("authorization")
                || stderr.contains("Permission denied")
                || stderr.contains("User canceled")
            {
                return Err(CertError::PermissionDenied);
            }

            return Err(CertError::TrustStoreError(format!(
                "Failed to add CA to Keychain: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn remove_ca(&self) -> Result<(), CertError> {
        // Find the CA certificate by its common name
        let find_output = Command::new("security")
            .args([
                "find-certificate",
                "-c",
                ROXY_CA_NAME,
                "-a",
                "-Z",
                "/Library/Keychains/System.keychain",
            ])
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to find CA certificate: {}", e))
            })?;

        if !find_output.status.success() {
            let stderr = String::from_utf8_lossy(&find_output.stderr);
            if stderr.contains("could not be found")
                || stderr.contains("The specified item could not be found")
            {
                return Ok(());
            }

            if stderr.contains("authorization") || stderr.contains("Permission denied") {
                return Err(CertError::PermissionDenied);
            }

            return Err(CertError::TrustStoreError(format!(
                "Failed to find CA certificate: {}",
                stderr
            )));
        }

        // Parse the SHA-1 hash from output
        let stdout = String::from_utf8_lossy(&find_output.stdout);
        let hash = stdout
            .lines()
            .find(|line| line.starts_with("SHA-1 hash:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|h| h.trim());

        if let Some(hash) = hash {
            let delete_output = Command::new("security")
                .args([
                    "delete-certificate",
                    "-Z",
                    hash,
                    "/Library/Keychains/System.keychain",
                ])
                .output()
                .map_err(|e| {
                    CertError::TrustStoreError(format!("Failed to delete CA certificate: {}", e))
                })?;

            if !delete_output.status.success() {
                let stderr = String::from_utf8_lossy(&delete_output.stderr);

                if stderr.contains("authorization") || stderr.contains("Permission denied") {
                    return Err(CertError::PermissionDenied);
                }

                if !stderr.contains("could not be found") {
                    return Err(CertError::TrustStoreError(format!(
                        "Failed to delete CA certificate: {}",
                        stderr
                    )));
                }
            }
        }

        Ok(())
    }

    fn is_ca_trusted(&self) -> Result<bool, CertError> {
        let output = Command::new("security")
            .args([
                "find-certificate",
                "-c",
                ROXY_CA_NAME,
                "/Library/Keychains/System.keychain",
            ])
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to check CA certificate: {}", e))
            })?;

        Ok(output.status.success())
    }
}

impl Default for MacOsTrustStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_store_creation() {
        let _store = MacOsTrustStore::new();
    }
}
