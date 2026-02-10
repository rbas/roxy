use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::ServerConfig;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;
use tracing::warn;

use crate::domain::DomainName;

/// Custom certificate resolver that selects certificates based on SNI hostname.
/// Returns None for unknown domains, causing a clean TLS failure rather than
/// serving a mismatched certificate.
#[derive(Debug)]
struct DomainCertResolver {
    certs: HashMap<String, Arc<CertifiedKey>>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        let hostname = client_hello.server_name()?;

        let cert = self.certs.get(hostname).cloned();
        if cert.is_none() {
            warn!(hostname = %hostname, "TLS: no certificate for domain");
        }
        cert
    }
}

/// Load all domain certificates into a single TLS acceptor with SNI
pub fn create_tls_acceptor(
    domains: &[DomainName],
    certs_dir: &Path,
) -> Result<Option<TlsAcceptor>> {
    if domains.is_empty() {
        return Ok(None);
    }

    let mut certs_map = HashMap::new();

    // Load all domain certificates directly from certs_dir
    for domain in domains {
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
        certs_map.insert(domain.as_str().to_string(), certified_key);
    }

    let resolver = Arc::new(DomainCertResolver { certs: certs_map });

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);

    Ok(Some(TlsAcceptor::from(Arc::new(config))))
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
