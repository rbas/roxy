use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{Context, Result};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, Issuer, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, SanType,
};
use rustls::ServerConfig;
use rustls::pki_types::PrivateKeyDer;
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;
use tracing::warn;

use crate::domain::DomainName;
use crate::infrastructure::certs::generator::build_ca_cert_params;
use time::{Duration, OffsetDateTime};

const ON_DEMAND_CERT_CACHE_MAX: usize = 256;

/// Custom certificate resolver that selects certificates based on SNI hostname.
///
/// For unknown `.roxy` domains, we generate an on-demand certificate signed by
/// Roxy's local Root CA. This allows the HTTP layer to render a friendly
/// "Domain Not Registered" page instead of the browser showing a TLS error.
#[derive(Debug)]
struct DomainCertResolver {
    ca_key_pem: String,
    on_demand: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        let hostname = client_hello.server_name()?.to_lowercase();

        // Try cached on-demand certs first (for unregistered but valid .roxy domains).
        if let Some(cert) = self.on_demand.read().ok()?.get(hostname.as_str()).cloned() {
            return Some(cert);
        }

        // Generate an on-demand cert for valid `.roxy` hostnames if we
        // can read the local CA private key.
        if DomainName::new(hostname.as_str()).is_err() {
            warn!(hostname = %hostname, "TLS: no certificate for domain");
            return None;
        }

        match generate_on_demand_certified_key(hostname.as_str(), &self.ca_key_pem) {
            Ok(cert) => {
                if let Ok(mut cache) = self.on_demand.write() {
                    // Bound memory: on-demand certs are cheap to regenerate.
                    if cache.len() >= ON_DEMAND_CERT_CACHE_MAX {
                        cache.clear();
                    }
                    cache.insert(hostname, cert.clone());
                }
                Some(cert)
            }
            Err(e) => {
                warn!(hostname = %hostname, error = %e, "TLS: failed to generate on-demand certificate");
                None
            }
        }
    }
}

/// Create a TLS acceptor that generates exact leaf certificates from SNI.
///
/// The Root CA is trusted once during installation. Domain registration does
/// not create files or mutate a trust store.
pub fn create_tls_acceptor(data_dir: &Path) -> Result<Option<TlsAcceptor>> {
    let ca_key_pem = match load_ca_key_pem(data_dir) {
        Ok(Some(pem)) => pem,
        Ok(None) => return Ok(None),
        Err(e) => {
            warn!(error = %e, "TLS: failed to load Roxy CA key (on-demand certificates disabled)");
            return Ok(None);
        }
    };

    let resolver = Arc::new(DomainCertResolver {
        ca_key_pem,
        on_demand: RwLock::new(HashMap::new()),
    });

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);

    Ok(Some(TlsAcceptor::from(Arc::new(config))))
}

fn load_ca_key_pem(data_dir: &Path) -> Result<Option<String>> {
    let ca_key_path = data_dir.join("ca.key");
    if !ca_key_path.exists() {
        return Ok(None);
    }

    let pem = fs::read_to_string(&ca_key_path)
        .with_context(|| format!("Failed to read CA private key: {}", ca_key_path.display()))?;
    Ok(Some(pem))
}

fn generate_on_demand_certified_key(hostname: &str, ca_key_pem: &str) -> Result<Arc<CertifiedKey>> {
    let leaf_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .context("Failed to generate leaf key pair")?;

    let san = SanType::DnsName(
        hostname
            .try_into()
            .map_err(|e| anyhow::anyhow!("Invalid hostname for SAN: {}", e))?,
    );
    let params = build_leaf_cert_params(hostname, vec![san]);

    let ca_key_pair = KeyPair::from_pem(ca_key_pem).context("Failed to parse CA key")?;
    let ca_params = build_ca_cert_params();
    let issuer = Issuer::from_params(&ca_params, &ca_key_pair);

    let cert = params
        .signed_by(&leaf_key_pair, &issuer)
        .context("Failed to sign on-demand certificate")?;

    let certs = vec![cert.der().clone()];
    let key = PrivateKeyDer::try_from(leaf_key_pair.serialize_der())
        .map_err(|e| anyhow::anyhow!("Failed to parse generated private key: {}", e))?;

    let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
        .context("Failed to create signing key")?;

    Ok(Arc::new(CertifiedKey::new(certs, signing_key)))
}

fn build_leaf_cert_params(common_name: &str, sans: Vec<SanType>) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.subject_alt_names = sans;
    let mut distinguished_name = DistinguishedName::new();
    distinguished_name.push(DnType::CommonName, common_name);
    params.distinguished_name = distinguished_name;
    params.not_before = OffsetDateTime::now_utc() - Duration::days(1);
    params.not_after = OffsetDateTime::now_utc() + Duration::days(825);
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_ca_disables_tls() {
        let directory = tempfile::tempdir().unwrap();
        assert!(create_tls_acceptor(directory.path()).unwrap().is_none());
    }

    #[test]
    fn creates_an_in_memory_leaf_key() {
        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let certified =
            generate_on_demand_certified_key("app.roxy", &ca_key.serialize_pem()).unwrap();

        assert_eq!(certified.cert.len(), 1);
        assert!(!certified.cert[0].as_ref().is_empty());
    }

    #[test]
    fn invalid_ca_key_is_rejected() {
        let error = generate_on_demand_certified_key("app.roxy", "not a key").unwrap_err();
        assert!(error.to_string().contains("Failed to parse CA key"));
    }

    #[tokio::test]
    async fn generated_leaf_chains_to_a_legacy_installed_ca() {
        use rustls::pki_types::ServerName;
        use rustls::{ClientConfig, RootCertStore};
        use tokio_rustls::TlsConnector;

        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let mut legacy_params = CertificateParams::default();
        let mut legacy_name = DistinguishedName::new();
        legacy_name.push(DnType::CommonName, "Roxy Local Development CA");
        legacy_name.push(DnType::OrganizationName, "Roxy");
        legacy_params.distinguished_name = legacy_name;
        legacy_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        legacy_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        let ca_cert = legacy_params.self_signed(&ca_key).unwrap();

        let resolver = Arc::new(DomainCertResolver {
            ca_key_pem: ca_key.serialize_pem(),
            on_demand: RwLock::new(HashMap::new()),
        });
        let server = TlsAcceptor::from(Arc::new(
            ServerConfig::builder()
                .with_no_client_auth()
                .with_cert_resolver(resolver),
        ));

        let mut roots = RootCertStore::empty();
        roots.add(ca_cert.der().clone()).unwrap();
        let client = TlsConnector::from(Arc::new(
            ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth(),
        ));
        let server_name = ServerName::try_from("app.roxy").unwrap();
        let (client_io, server_io) = tokio::io::duplex(16 * 1024);

        let (client_result, server_result) = tokio::join!(
            client.connect(server_name, client_io),
            server.accept(server_io)
        );
        client_result.unwrap();
        server_result.unwrap();
    }
}
