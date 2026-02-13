use rcgen::{
    CertificateParams, DistinguishedName, DnType, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
    SanType,
};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

use super::CertError;
use super::ca::RootCA;
use super::WILDCARD_CERT_PREFIX;
use crate::domain::DomainName;

/// Represents a generated certificate with its key pair
pub struct Certificate {
    pub domain: String,
    pub cert_pem: String,
    pub key_pem: String,
}

pub struct CertificateGenerator {
    base_dir: PathBuf,
    certs_dir: PathBuf,
}

impl CertificateGenerator {
    /// Create a CertificateGenerator with explicit directories
    pub fn new(base_dir: PathBuf, certs_dir: PathBuf) -> Self {
        Self {
            base_dir,
            certs_dir,
        }
    }

    /// Generate a new certificate for the given domain, signed by the Root CA
    pub fn generate(&self, domain: &DomainName) -> Result<Certificate, CertError> {
        let ca = RootCA::new(self.base_dir.clone());

        // Ensure CA exists
        if !ca.exists() {
            return Err(CertError::GenerationError(
                "Root CA not found. Run 'sudo roxy install' first.".to_string(),
            ));
        }

        let domain_str = domain.as_str();

        // Generate ECDSA P-256 key pair for the domain (per FR-3.1.2)
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

        // Set key usage for server certificate
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        // Sign certificate with CA
        let cert_pem = ca.sign_certificate(params, &key_pair)?;

        Ok(Certificate {
            domain: domain_str.to_string(),
            cert_pem,
            key_pem: key_pair.serialize_pem(),
        })
    }

    /// Generate a wildcard certificate for the given base domain, signed by the Root CA.
    ///
    /// The certificate includes SANs for:
    /// - base (myapp.roxy)
    /// - wildcard (*.myapp.roxy)
    pub fn generate_wildcard(&self, base_domain: &DomainName) -> Result<Certificate, CertError> {
        let ca = RootCA::new(self.base_dir.clone());

        // Ensure CA exists
        if !ca.exists() {
            return Err(CertError::GenerationError(
                "Root CA not found. Run 'sudo roxy install' first.".to_string(),
            ));
        }

        let base_str = base_domain.as_str();

        // Generate ECDSA P-256 key pair for the domain (per FR-3.1.2)
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        // Configure certificate parameters
        let mut params = CertificateParams::default();

        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, base_str);
        dn.push(DnType::OrganizationName, "Roxy Local Development");
        params.distinguished_name = dn;

        // Set validity period (1 year per FR-3.1.3)
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(365);

        // Add Subject Alternative Names for base + wildcard.
        let wildcard_str = format!("*.{}", base_str);
        params.subject_alt_names = vec![
            SanType::DnsName(base_str.try_into().map_err(|e| {
                CertError::GenerationError(format!("Invalid domain name for SAN: {}", e))
            })?),
            SanType::DnsName(wildcard_str.try_into().map_err(|e| {
                CertError::GenerationError(format!("Invalid wildcard name for SAN: {}", e))
            })?),
        ];

        // Set key usage for server certificate
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];

        // Sign certificate with CA
        let cert_pem = ca.sign_certificate(params, &key_pair)?;

        Ok(Certificate {
            domain: format!("{}{}", WILDCARD_CERT_PREFIX, base_str),
            cert_pem,
            key_pem: key_pair.serialize_pem(),
        })
    }

    /// Save a certificate to disk
    pub fn save(&self, cert: &Certificate) -> Result<(), CertError> {
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

        Ok(())
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

    /// Delete wildcard certificate files for a base domain.
    pub fn delete_wildcard(&self, base_domain: &DomainName) -> Result<(), CertError> {
        let base_str = base_domain.as_str();
        let cert_path = self
            .certs_dir
            .join(format!("{}{}.crt", WILDCARD_CERT_PREFIX, base_str));
        let key_path = self
            .certs_dir
            .join(format!("{}{}.key", WILDCARD_CERT_PREFIX, base_str));

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

    /// Check if certificate exists for a domain
    pub fn exists(&self, domain: &DomainName) -> bool {
        let cert_path = self.certs_dir.join(format!("{}.crt", domain.as_str()));
        let key_path = self.certs_dir.join(format!("{}.key", domain.as_str()));
        cert_path.exists() && key_path.exists()
    }

    /// Check if a wildcard certificate exists for a base domain.
    pub fn exists_wildcard(&self, base_domain: &DomainName) -> bool {
        let base_str = base_domain.as_str();
        let cert_path = self
            .certs_dir
            .join(format!("{}{}.crt", WILDCARD_CERT_PREFIX, base_str));
        let key_path = self
            .certs_dir
            .join(format!("{}{}.key", WILDCARD_CERT_PREFIX, base_str));
        cert_path.exists() && key_path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_certificate_generation() {
        // Create an isolated test environment
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_dir = temp_dir.path().to_path_buf();
        let certs_dir = base_dir.join("certs");

        // Generate a test CA in the temp directory
        let ca = RootCA::new(base_dir.clone());
        ca.generate().expect("Failed to generate test CA");

        // Now test certificate generation
        let domain = DomainName::new("test.roxy").unwrap();
        let generator = CertificateGenerator::new(base_dir, certs_dir);

        let cert = generator.generate(&domain).unwrap();

        assert_eq!(cert.domain, "test.roxy");
        assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.key_pem.contains("BEGIN PRIVATE KEY"));
    }
}
