use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    PKCS_ECDSA_P256_SHA256,
};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

use super::CertError;

/// Roxy Root Certificate Authority
///
/// This CA is used to sign all domain certificates. Users only need to trust
/// the CA once, and all domain certificates will be automatically trusted.
pub struct RootCA {
    roxy_dir: PathBuf,
}

impl RootCA {
    pub fn new() -> Self {
        let roxy_dir = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".roxy");

        Self { roxy_dir }
    }

    /// Create a RootCA with a custom base directory (useful for testing)
    pub fn with_base_dir(roxy_dir: PathBuf) -> Self {
        Self { roxy_dir }
    }

    /// Path to the CA certificate
    pub fn cert_path(&self) -> PathBuf {
        self.roxy_dir.join("ca.crt")
    }

    /// Path to the CA private key
    pub fn key_path(&self) -> PathBuf {
        self.roxy_dir.join("ca.key")
    }

    /// Check if the CA already exists
    pub fn exists(&self) -> bool {
        self.cert_path().exists() && self.key_path().exists()
    }

    /// Generate a new Root CA certificate
    pub fn generate(&self) -> Result<(), CertError> {
        // Ensure directory exists
        fs::create_dir_all(&self.roxy_dir).map_err(|e| CertError::WriteError {
            path: self.roxy_dir.clone(),
            source: e,
        })?;

        // Generate ECDSA P-256 key pair
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Configure CA certificate parameters
        let mut params = CertificateParams::default();

        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Roxy Local Development CA");
        dn.push(DnType::OrganizationName, "Roxy");
        params.distinguished_name = dn;

        // Set validity period (10 years for CA)
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(3650);

        // Mark as CA certificate
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];

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

        // Create CA certificate params to sign with
        let mut ca_params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Roxy Local Development CA");
        dn.push(DnType::OrganizationName, "Roxy");
        ca_params.distinguished_name = dn;

        let now = OffsetDateTime::now_utc();
        ca_params.not_before = now;
        ca_params.not_after = now + Duration::days(3650);
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

        // Generate CA cert from params
        let ca_cert = ca_params
            .self_signed(&ca_key_pair)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Sign the domain certificate
        let cert = params
            .signed_by(key_pair, &ca_cert, &ca_key_pair)
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

impl Default for RootCA {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_paths() {
        let ca = RootCA::new();
        assert!(ca.cert_path().to_string_lossy().contains("ca.crt"));
        assert!(ca.key_path().to_string_lossy().contains("ca.key"));
    }
}
