use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use anyhow::{Context, Result};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, Issuer, KeyPair, KeyUsagePurpose,
    PKCS_ECDSA_P256_SHA256, SanType,
};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;
use time::{Duration, OffsetDateTime};
use tracing::warn;

use crate::domain::DomainName;
use crate::infrastructure::certs::WILDCARD_CERT_PREFIX;

const ON_DEMAND_CERT_CACHE_MAX: usize = 256;

/// Custom certificate resolver that selects certificates based on SNI hostname.
///
/// For unknown `.roxy` domains, we generate an on-demand certificate signed by
/// Roxy's local Root CA. This allows the HTTP layer to render a friendly
/// "Domain Not Registered" page instead of the browser showing a TLS error.
#[derive(Debug)]
struct DomainCertResolver {
    exact_certs: HashMap<String, Arc<CertifiedKey>>,
    wildcard_certs: Vec<(String, Arc<CertifiedKey>)>,
    ca_key_pem: Option<String>,
    on_demand: RwLock<HashMap<String, Arc<CertifiedKey>>>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        let hostname = client_hello.server_name()?.to_lowercase();

        // Prefer an exact per-domain cert (generated during `roxy register`).
        if let Some(cert) = self.exact_certs.get(hostname.as_str()).cloned() {
            return Some(cert);
        }

        // Then try cached on-demand certs.
        if let Some(cert) = self.on_demand.read().ok()?.get(hostname.as_str()).cloned() {
            return Some(cert);
        }

        // Then try wildcard certificates (generated during `roxy register --wildcard`).
        for (base, cert) in &self.wildcard_certs {
            if wildcard_cert_covers_hostname(base.as_str(), hostname.as_str()) {
                return Some(cert.clone());
            }
        }

        // Finally, generate an on-demand cert for valid `.roxy` hostnames if we
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
    exact_domains: &[DomainName],
    wildcard_domains: &[DomainName],
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

    // If we have neither per-domain certificates nor a Root CA to generate on-demand
    // certificates, HTTPS can't be served.
    if exact_domains.is_empty() && wildcard_domains.is_empty() && ca_key_pem.is_none() {
        return Ok(None);
    }

    let mut exact_certs = HashMap::new();

    // Load all domain certificates directly from certs_dir
    for domain in exact_domains {
        let domain_str = domain.as_str();
        let cert_path = certs_dir.join(format!("{}.crt", domain_str));
        let key_path = certs_dir.join(format!("{}.key", domain_str));

        if !cert_path.exists() || !key_path.exists() {
            anyhow::bail!("No certificate found for {}", domain);
        }

        let certs = load_certs(&cert_path)?;
        let key = load_private_key(&key_path)?;

        // Create a signing key using aws-lc-rs (default crypto provider)
        let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
            .context("Failed to create signing key")?;

        let certified_key = Arc::new(CertifiedKey::new(certs, signing_key));
        exact_certs.insert(domain.as_str().to_string(), certified_key);
    }

    // Load wildcard certificates (base + *.base SAN) for wildcard registrations.
    let mut wildcard_certs = Vec::new();
    for base in wildcard_domains {
        let base_str = base.as_str();
        let cert_path = certs_dir.join(format!("{WILDCARD_CERT_PREFIX}{base_str}.crt"));
        let key_path = certs_dir.join(format!("{WILDCARD_CERT_PREFIX}{base_str}.key"));

        if !cert_path.exists() || !key_path.exists() {
            anyhow::bail!("No wildcard certificate found for *.{}", base);
        }

        let certs = load_certs(&cert_path)?;
        let key = load_private_key(&key_path)?;

        let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
            .context("Failed to create signing key")?;

        let certified_key = Arc::new(CertifiedKey::new(certs, signing_key));
        wildcard_certs.push((base_str.to_string(), certified_key));
    }

    // Most-specific wildcard wins (longest base domain).
    wildcard_certs.sort_by_key(|(base, _)| std::cmp::Reverse(base.len()));

    let resolver = Arc::new(DomainCertResolver {
        exact_certs,
        wildcard_certs,
        ca_key_pem,
        on_demand: RwLock::new(HashMap::new()),
    });

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);

    Ok(Some(TlsAcceptor::from(Arc::new(config))))
}

fn wildcard_cert_covers_hostname(base: &str, hostname: &str) -> bool {
    // Our wildcard certs include both:
    // - base (myapp.roxy)
    // - *.base (covers one-label subdomains like blog.myapp.roxy)
    if hostname == base {
        return true;
    }

    let suffix = format!(".{}", base);
    if !hostname.ends_with(&suffix) {
        return false;
    }

    // Ensure only one label before the base.
    let prefix = &hostname[..hostname.len() - suffix.len()];
    !prefix.is_empty() && !prefix.contains('.')
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
    // Generate a leaf keypair for this hostname.
    let leaf_key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
        .context("Failed to generate leaf key pair")?;

    // Configure certificate parameters (aligned with infrastructure/certs/generator.rs).
    let mut params = CertificateParams::default();
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, hostname);
    dn.push(DnType::OrganizationName, "Roxy Local Development");
    params.distinguished_name = dn;

    let now = OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + Duration::days(365);

    params.subject_alt_names = vec![SanType::DnsName(hostname.try_into().map_err(|e| {
        anyhow::anyhow!("Invalid hostname for SAN: {}", e)
    })?)];

    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];

    // Build a CA issuer using the on-disk Root CA private key.
    let ca_key_pair = KeyPair::from_pem(ca_key_pem).context("Failed to parse CA key")?;
    let mut ca_params = CertificateParams::default();
    let mut ca_dn = DistinguishedName::new();
    ca_dn.push(DnType::CommonName, "Roxy Local Development CA");
    ca_dn.push(DnType::OrganizationName, "Roxy");
    ca_params.distinguished_name = ca_dn;
    ca_params.not_before = now;
    ca_params.not_after = now + Duration::days(3650);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

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
