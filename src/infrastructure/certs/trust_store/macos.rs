use std::path::Path;
use std::process::Command;

use super::super::CertError;
use super::TrustStore;
use crate::domain::DomainName;

/// macOS Keychain trust store implementation
pub struct MacOsTrustStore;

impl MacOsTrustStore {
    pub fn new() -> Self {
        Self
    }
}

impl TrustStore for MacOsTrustStore {
    fn add_certificate(&self, cert_path: &Path, _domain: &DomainName) -> Result<(), CertError> {
        // Add certificate to system keychain
        // security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <cert>
        let output = Command::new("security")
            .args([
                "add-trusted-cert",
                "-d", // Add to admin cert store
                "-r",
                "trustRoot", // Trust as root certificate
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

            // Check for permission error
            if stderr.contains("authorization")
                || stderr.contains("Permission denied")
                || stderr.contains("User canceled")
            {
                return Err(CertError::PermissionDenied);
            }

            return Err(CertError::TrustStoreError(format!(
                "Failed to add certificate to Keychain: {}",
                stderr
            )));
        }

        Ok(())
    }

    fn remove_certificate(&self, domain: &DomainName) -> Result<(), CertError> {
        // First, find the certificate hash by its common name
        // security find-certificate -c "<domain>" -a -Z /Library/Keychains/System.keychain
        let find_output = Command::new("security")
            .args([
                "find-certificate",
                "-c",
                domain.as_str(),
                "-a",
                "-Z",
                "/Library/Keychains/System.keychain",
            ])
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to find certificate: {}", e))
            })?;

        if !find_output.status.success() {
            // Certificate not found is not an error during removal
            let stderr = String::from_utf8_lossy(&find_output.stderr);
            if stderr.contains("could not be found")
                || stderr.contains("The specified item could not be found")
            {
                return Ok(());
            }

            // Check for permission error
            if stderr.contains("authorization") || stderr.contains("Permission denied") {
                return Err(CertError::PermissionDenied);
            }

            return Err(CertError::TrustStoreError(format!(
                "Failed to find certificate: {}",
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
            // Delete the certificate by hash
            // security delete-certificate -Z <hash> /Library/Keychains/System.keychain
            let delete_output = Command::new("security")
                .args([
                    "delete-certificate",
                    "-Z",
                    hash,
                    "/Library/Keychains/System.keychain",
                ])
                .output()
                .map_err(|e| {
                    CertError::TrustStoreError(format!("Failed to delete certificate: {}", e))
                })?;

            if !delete_output.status.success() {
                let stderr = String::from_utf8_lossy(&delete_output.stderr);

                if stderr.contains("authorization") || stderr.contains("Permission denied") {
                    return Err(CertError::PermissionDenied);
                }

                // Not found is OK
                if !stderr.contains("could not be found") {
                    return Err(CertError::TrustStoreError(format!(
                        "Failed to delete certificate: {}",
                        stderr
                    )));
                }
            }
        }

        Ok(())
    }

    fn is_trusted(&self, domain: &DomainName) -> Result<bool, CertError> {
        // Check if certificate exists in system keychain
        let output = Command::new("security")
            .args([
                "find-certificate",
                "-c",
                domain.as_str(),
                "/Library/Keychains/System.keychain",
            ])
            .output()
            .map_err(|e| {
                CertError::TrustStoreError(format!("Failed to check certificate: {}", e))
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
