use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair, PKCS_ECDSA_P256_SHA256};
use std::fs;
use std::path::PathBuf;

use super::CertError;

/// Roxy Root Certificate Authority
///
/// This CA is used to sign all domain certificates. Users only need to trust
/// the CA once, and all domain certificates will be automatically trusted.
pub struct RootCA {
    data_dir: PathBuf,
}

impl RootCA {
    /// Create a RootCA with the given base directory
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Path to the CA certificate
    pub fn cert_path(&self) -> PathBuf {
        self.data_dir.join("ca.crt")
    }

    /// Path to the CA private key
    pub fn key_path(&self) -> PathBuf {
        self.data_dir.join("ca.key")
    }

    /// Check if the CA already exists
    pub fn exists(&self) -> bool {
        self.cert_path().exists() && self.key_path().exists()
    }

    /// Generate a new Root CA certificate
    pub fn generate(&self) -> Result<(), CertError> {
        // Ensure directory exists
        fs::create_dir_all(&self.data_dir).map_err(|e| CertError::WriteError {
            path: self.data_dir.clone(),
            source: e,
        })?;

        // Generate ECDSA P-256 key pair
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Configure CA certificate parameters (shared DN + validity + key usage)
        let mut params = super::generator::build_ca_cert_params();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);

        // Generate the self-signed CA certificate
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Save certificate
        let cert_path = self.cert_path();
        fs::write(&cert_path, cert.pem()).map_err(|e| CertError::WriteError {
            path: cert_path.clone(),
            source: e,
        })?;

        // Save private key with restricted permissions
        let key_path = self.key_path();
        fs::write(&key_path, key_pair.serialize_pem()).map_err(|e| CertError::WriteError {
            path: key_path.clone(),
            source: e,
        })?;

        // Set key file permissions to 0600 (owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&key_path)
                .map_err(|e| CertError::WriteError {
                    path: key_path.clone(),
                    source: e,
                })?
                .permissions();
            perms.set_mode(0o600);
            fs::set_permissions(&key_path, perms).map_err(|e| CertError::WriteError {
                path: key_path.clone(),
                source: e,
            })?;
        }

        Ok(())
    }

    /// Load the CA key pair for signing
    pub fn load_key_pair(&self) -> Result<KeyPair, CertError> {
        let key_pem = fs::read_to_string(self.key_path()).map_err(|e| CertError::ReadError {
            path: self.key_path(),
            source: e,
        })?;

        KeyPair::from_pem(&key_pem).map_err(|e| CertError::GenerationError(e.to_string()))
    }

    /// Sign a certificate with this CA
    /// Returns the signed certificate PEM
    pub fn sign_certificate(
        &self,
        params: CertificateParams,
        key_pair: &KeyPair,
    ) -> Result<String, CertError> {
        let ca_key_pair = self.load_key_pair()?;

        let ca_params = super::generator::build_ca_cert_params();
        let issuer = Issuer::from_params(&ca_params, &ca_key_pair);

        // Sign the domain certificate
        let cert = params
            .signed_by(key_pair, &issuer)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        Ok(cert.pem())
    }

    /// Delete the CA certificate and key (used by uninstall)
    pub fn delete(&self) -> Result<(), CertError> {
        let cert_path = self.cert_path();
        let key_path = self.key_path();

        if cert_path.exists() {
            fs::remove_file(&cert_path).map_err(|e| CertError::DeleteError {
                path: cert_path,
                source: e,
            })?;
        }

        if key_path.exists() {
            fs::remove_file(&key_path).map_err(|e| CertError::DeleteError {
                path: key_path,
                source: e,
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_paths() {
        let ca = RootCA::new(PathBuf::from("/tmp/test-roxy"));
        assert!(ca.cert_path().to_string_lossy().contains("ca.crt"));
        assert!(ca.key_path().to_string_lossy().contains("ca.key"));
    }
}
