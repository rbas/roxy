use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{Context, Result};
use rcgen::{Issuer, KeyPair, PKCS_ECDSA_P256_SHA256, SanType};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;
use tracing::warn;

use crate::domain::{DomainName, DomainPattern};
use crate::infrastructure::certs::generator::{build_ca_cert_params, build_leaf_cert_params};

const ON_DEMAND_CERT_CACHE_MAX: usize = 256;

/// Custom certificate resolver that selects certificates based on SNI hostname.
///
/// For unknown `.roxy` domains, we generate an on-demand certificate signed by
/// Roxy's local Root CA. This allows the HTTP layer to render a friendly
/// "Domain Not Registered" page instead of the browser showing a TLS error.
#[derive(Debug)]
struct DomainCertResolver {
    /// All registered certificates, stored with their pattern for matching.
    certs: Vec<(DomainPattern, Arc<CertifiedKey>)>,
    ca_key_pem: Option<String>,
    on_demand: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        let hostname = client_hello.server_name()?.to_lowercase();

        // Try cached on-demand certs first (for unregistered but valid .roxy domains).
        if let Some(cert) = self.on_demand.read().ok()?.get(hostname.as_str()).cloned() {
            return Some(cert);
        }

        // Find the first registered cert whose pattern matches the hostname.
        // Certs are pre-sorted by specificity (most specific first).
        for (pattern, cert) in &self.certs {
            if pattern.matches_hostname(&hostname) {
                return Some(cert.clone());
            }
        }

        // Generate an on-demand cert for valid `.roxy` hostnames if we
        // can read the local CA private key.
        let ca_key_pem = self.ca_key_pem.as_deref()?;
        if DomainName::new(hostname.as_str()).is_err() {
            warn!(hostname = %hostname, "TLS: no certificate for domain");
            return None;
        }

        match generate_on_demand_certified_key(hostname.as_str(), ca_key_pem) {
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

/// Load all domain certificates into a single TLS acceptor with SNI
pub fn create_tls_acceptor(
    patterns: &[DomainPattern],
    certs_dir: &Path,
    data_dir: &Path,
) -> Result<Option<TlsAcceptor>> {
    let ca_key_pem = match load_ca_key_pem(data_dir) {
        Ok(pem) => pem,
        Err(e) => {
            warn!(error = %e, "TLS: failed to load Roxy CA key (on-demand certificates disabled)");
            None
        }
    };

    // If we have neither per-domain certificates nor a Root CA to generate
    // on-demand certificates, HTTPS can't be served.
    if patterns.is_empty() && ca_key_pem.is_none() {
        return Ok(None);
    }

    let mut certs: Vec<(DomainPattern, Arc<CertifiedKey>)> = Vec::new();

    for pattern in patterns {
        let stem = pattern.cert_name();
        let cert_path = certs_dir.join(format!("{}.crt", stem));
        let key_path = certs_dir.join(format!("{}.key", stem));

        if !cert_path.exists() || !key_path.exists() {
            anyhow::bail!("No certificate found for {}", pattern);
        }

        let loaded_certs = load_certs(&cert_path)?;
        let key = load_private_key(&key_path)?;

        let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
            .context("Failed to create signing key")?;

        let certified_key = Arc::new(CertifiedKey::new(loaded_certs, signing_key));
        certs.push((pattern.clone(), certified_key));
    }

    // Most-specific pattern wins (longest base domain).
    certs.sort_by_key(|(p, _)| std::cmp::Reverse(p.specificity()));

    let resolver = Arc::new(DomainCertResolver {
        certs,
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

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let certs: Vec<_> = CertificateDer::pem_file_iter(path)
        .with_context(|| format!("Failed to open cert file: {}", path.display()))?
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;

    Ok(certs)
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    PrivateKeyDer::from_pem_file(path)
        .with_context(|| format!("Failed to load private key from: {}", path.display()))
}
