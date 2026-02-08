use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::ResolvesServerCert;
use rustls::sign::CertifiedKey;
use tokio_rustls::TlsAcceptor;

use crate::domain::DomainName;
use crate::infrastructure::certs::CertificateGenerator;

/// Custom certificate resolver that selects certificates based on SNI hostname
#[derive(Debug)]
struct DomainCertResolver {
    certs: HashMap<String, Arc<CertifiedKey>>,
    fallback: Arc<CertifiedKey>,
}

impl ResolvesServerCert for DomainCertResolver {
    fn resolve(&self, client_hello: rustls::server::ClientHello) -> Option<Arc<CertifiedKey>> {
        // Get the SNI hostname from the client hello
        let hostname = client_hello.server_name()?;

        // Look up certificate for this domain, fall back to first cert if not found
        self.certs
            .get(hostname)
            .cloned()
            .or_else(|| Some(self.fallback.clone()))
    }
}

/// Load all domain certificates into a single TLS acceptor with SNI
pub fn create_tls_acceptor(domains: &[DomainName]) -> Result<Option<TlsAcceptor>> {
    if domains.is_empty() {
        return Ok(None);
    }

    let mut certs_map = HashMap::new();
    let generator = CertificateGenerator::new();

    // Load all domain certificates
    for domain in domains {
        let paths = generator
            .get_paths(domain)
            .ok_or_else(|| anyhow::anyhow!("No certificate found for {}", domain))?;

        let certs = load_certs(&paths.cert)?;
        let key = load_private_key(&paths.key)?;

        // Create a signing key using aws-lc-rs (default crypto provider)
        let signing_key = rustls::crypto::aws_lc_rs::sign::any_supported_type(&key)
            .context("Failed to create signing key")?;

        let certified_key = Arc::new(CertifiedKey::new(certs, signing_key));
        certs_map.insert(domain.as_str().to_string(), certified_key);
    }

    // Use the first domain's cert as fallback
    let fallback = certs_map
        .get(domains[0].as_str())
        .ok_or_else(|| anyhow::anyhow!("No fallback certificate available"))?
        .clone();

    let resolver = Arc::new(DomainCertResolver {
        certs: certs_map,
        fallback,
    });

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_cert_resolver(resolver);

    Ok(Some(TlsAcceptor::from(Arc::new(config))))
}

fn load_certs(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open cert file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let certs: Vec<_> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse certificates")?;

    Ok(certs)
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let file =
        File::open(path).with_context(|| format!("Failed to open key file: {}", path.display()))?;
    let mut reader = BufReader::new(file);

    let key = rustls_pemfile::private_key(&mut reader)
        .context("Failed to parse private key")?
        .ok_or_else(|| anyhow::anyhow!("No private key found in file"))?;

    Ok(key)
}
