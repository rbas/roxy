use rcgen::{
    CertificateParams, DistinguishedName, DnType, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
    SanType,
};
use std::fs;
use std::path::PathBuf;
use time::{Duration, OffsetDateTime};

use super::CertError;
use super::ca::RootCA;
use crate::domain::DomainPattern;

/// Represents a generated certificate with its key pair
pub struct Certificate {
    /// File stem used for saving (e.g. "myapp.roxy" or "__wildcard__.myapp.roxy")
    pub file_stem: String,
    pub cert_pem: String,
    pub key_pem: String,
}

/// Build certificate parameters for a domain leaf certificate.
///
/// Sets up the Distinguished Name, 1-year validity, key usage for
/// server authentication, and the given Subject Alternative Names.
pub(crate) fn build_leaf_cert_params(common_name: &str, sans: Vec<SanType>) -> CertificateParams {
    let mut params = CertificateParams::default();

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, common_name);
    dn.push(DnType::OrganizationName, "Roxy Local Development");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + Duration::days(365);

    params.subject_alt_names = sans;

    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    params
}

/// Build the standard Roxy CA certificate parameters.
pub(crate) fn build_ca_cert_params() -> CertificateParams {
    let mut params = CertificateParams::default();

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "Roxy Local Development CA");
    dn.push(DnType::OrganizationName, "Roxy");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + Duration::days(3650);

    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

    params
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

    /// Generate a certificate for the given domain pattern, signed by the Root CA.
    ///
    /// For exact patterns, generates a single-domain cert.
    /// For wildcard patterns, generates a cert with SANs for base + *.base.
    pub fn generate(&self, pattern: &DomainPattern) -> Result<Certificate, CertError> {
        let ca = RootCA::new(self.base_dir.clone());

        if !ca.exists() {
            return Err(CertError::GenerationError(
                "Root CA not found. Run 'sudo roxy install' first.".to_string(),
            ));
        }

        // Generate ECDSA P-256 key pair
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| CertError::GenerationError(e.to_string()))?;

        let sans = build_sans(pattern)?;
        let params = build_leaf_cert_params(pattern.base_domain().as_str(), sans);

        // Sign certificate with CA
        let cert_pem = ca.sign_certificate(params, &key_pair)?;

        Ok(Certificate {
            file_stem: pattern.cert_name(),
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

        let cert_path = self.certs_dir.join(format!("{}.crt", cert.file_stem));
        let key_path = self.certs_dir.join(format!("{}.key", cert.file_stem));

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

    /// Delete certificate files for a domain pattern
    pub fn delete(&self, pattern: &DomainPattern) -> Result<(), CertError> {
        let stem = pattern.cert_name();
        let cert_path = self.certs_dir.join(format!("{}.crt", stem));
        let key_path = self.certs_dir.join(format!("{}.key", stem));

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

    /// Check if certificate exists for a domain pattern
    pub fn exists(&self, pattern: &DomainPattern) -> bool {
        let stem = pattern.cert_name();
        let cert_path = self.certs_dir.join(format!("{}.crt", stem));
        let key_path = self.certs_dir.join(format!("{}.key", stem));
        cert_path.exists() && key_path.exists()
    }
}

/// Build Subject Alternative Names for the given pattern.
fn build_sans(pattern: &DomainPattern) -> Result<Vec<SanType>, CertError> {
    let base_str = pattern.base_domain().as_str();

    let base_san =
        SanType::DnsName(base_str.try_into().map_err(|e| {
            CertError::GenerationError(format!("Invalid domain name for SAN: {}", e))
        })?);

    match pattern {
        DomainPattern::Exact(_) => Ok(vec![base_san]),
        DomainPattern::Wildcard(_) => {
            let wildcard_str = format!("*.{}", base_str);
            let wildcard_san = SanType::DnsName(wildcard_str.try_into().map_err(|e| {
                CertError::GenerationError(format!("Invalid wildcard name for SAN: {}", e))
            })?);
            Ok(vec![base_san, wildcard_san])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DomainName;
    use crate::infrastructure::certs::ca::RootCA;
    use tempfile::TempDir;

    #[test]
    fn test_certificate_generation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let base_dir = temp_dir.path().to_path_buf();
        let certs_dir = base_dir.join("certs");

        let ca = RootCA::new(base_dir.clone());
        ca.generate().expect("Failed to generate test CA");

        let domain = DomainName::new("test.roxy").unwrap();
        let pattern = DomainPattern::Exact(domain);
        let generator = CertificateGenerator::new(base_dir, certs_dir);

        let cert = generator.generate(&pattern).unwrap();

        assert_eq!(cert.file_stem, "test.roxy");
        assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(cert.key_pem.contains("BEGIN PRIVATE KEY"));
    }
}
