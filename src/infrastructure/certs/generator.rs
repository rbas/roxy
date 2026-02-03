use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair, SanType, PKCS_ECDSA_P256_SHA256};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

use super::CertError;
use crate::domain::DomainName;

/// Represents a generated certificate with its key pair
pub struct Certificate {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
}

/// Paths to certificate and key files
#[allow(dead_code)]
pub struct CertPaths {
    pub cert: PathBuf,
    pub key: PathBuf,
}

pub struct CertificateGenerator {
    certs_dir: PathBuf,
}

impl CertificateGenerator {
    pub fn new() -> Self {
        let certs_dir = dirs::home_dir()
            .expect("Could not find home directory")
            .join(".roxy")
            .join("certs");

        Self { certs_dir }
    }

    /// Returns the directory where certificates are stored
    #[allow(dead_code)]
    pub fn certs_dir(&self) -> &PathBuf {
        &self.certs_dir
    }

    /// Generate a new certificate for the given domain
    pub fn generate(&self, domain: &DomainName) -> Result<Certificate, CertError> {
        let domain_str = domain.as_str();

        // Generate ECDSA P-256 key pair (per FR-3.1.2)
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Configure certificate parameters
        let mut params = CertificateParams::default();

        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, domain_str);
        dn.push(DnType::OrganizationName, "Roxy Local Development");
        params.distinguished_name = dn;

        // Set validity period (1 year per FR-3.1.3)
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(365);

        // Add Subject Alternative Name (FR-3.1.4)
        params.subject_alt_names = vec![SanType::DnsName(domain_str.try_into().map_err(|e| {
            CertError::GenerationError(format!("Invalid domain name for SAN: {}", e))
        })?)];

        // Generate the certificate
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        Ok(Certificate {
            domain: domain_str.to_string(),
            cert_pem: cert.pem(),
            key_pem: key_pair.serialize_pem(),
        })
    }

    /// Save a certificate to disk
    pub fn save(&self, cert: &Certificate) -> Result<CertPaths, CertError> {
        // Ensure certs directory exists
        fs::create_dir_all(&self.certs_dir).map_err(|e| CertError::WriteError {
            path: self.certs_dir.clone(),
            source: e,
        })?;

        let cert_path = self.certs_dir.join(format!("{}.crt", cert.domain));
        let key_path = self.certs_dir.join(format!("{}.key", cert.domain));

        // Write certificate
        fs::write(&cert_path, &cert.cert_pem).map_err(|e| CertError::WriteError {
            path: cert_path.clone(),
            source: e,
        })?;

        // Write private key with restricted permissions
        fs::write(&key_path, &cert.key_pem).map_err(|e| CertError::WriteError {
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

        Ok(CertPaths {
            cert: cert_path,
            key: key_path,
        })
    }

    /// Delete certificate files for a domain
    pub fn delete(&self, domain: &DomainName) -> Result<(), CertError> {
        let domain_str = domain.as_str();
        let cert_path = self.certs_dir.join(format!("{}.crt", domain_str));
        let key_path = self.certs_dir.join(format!("{}.key", domain_str));

        // Remove certificate file if exists
        if cert_path.exists() {
            fs::remove_file(&cert_path).map_err(|e| CertError::DeleteError {
                path: cert_path,
                source: e,
            })?;
        }

        // Remove key file if exists
        if key_path.exists() {
            fs::remove_file(&key_path).map_err(|e| CertError::DeleteError {
                path: key_path,
                source: e,
            })?;
        }

        Ok(())
    }

    /// Check if certificate exists for a domain
    pub fn exists(&self, domain: &DomainName) -> bool {
        let cert_path = self.certs_dir.join(format!("{}.crt", domain.as_str()));
        let key_path = self.certs_dir.join(format!("{}.key", domain.as_str()));
        cert_path.exists() && key_path.exists()
    }

    /// Get paths to certificate files for a domain
    #[allow(dead_code)]
    pub fn get_paths(&self, domain: &DomainName) -> Option<CertPaths> {
        let domain_str = domain.as_str();
        let cert_path = self.certs_dir.join(format!("{}.crt", domain_str));
        let key_path = self.certs_dir.join(format!("{}.key", domain_str));

        if cert_path.exists() && key_path.exists() {
            Some(CertPaths {
                cert: cert_path,
                key: key_path,
            })
        } else {
            None
        }
    }
}

impl Default for CertificateGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_certificate_generation() {
        let domain = DomainName::new("test.roxy").unwrap();
        let generator = CertificateGenerator::new();

        let cert = generator.generate(&domain).unwrap();

        assert_eq!(cert.domain, "test.roxy");
        assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.key_pem.contains("BEGIN PRIVATE KEY"));
    }
}
