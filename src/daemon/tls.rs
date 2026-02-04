use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio_rustls::TlsAcceptor;

use crate::domain::DomainName;
use crate::infrastructure::certs::CertificateGenerator;

/// Load TLS configuration for a domain
pub fn load_tls_config(domain: &DomainName) -> Result<ServerConfig> {
    let generator = CertificateGenerator::new();
    let paths = generator
        .get_paths(domain)
        .ok_or_else(|| anyhow::anyhow!("No certificate found for {}", domain))?;

    let certs = load_certs(&paths.cert)?;
    let key = load_private_key(&paths.key)?;

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .context("Failed to create TLS config")?;

    Ok(config)
}

/// Load all domain certificates into a single TLS acceptor with SNI
pub fn create_tls_acceptor(domains: &[DomainName]) -> Result<Option<TlsAcceptor>> {
    if domains.is_empty() {
        return Ok(None);
    }

    // For simplicity, use the first domain's cert
    // TODO: Implement proper SNI with multiple certs
    let config = load_tls_config(&domains[0])?;
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
