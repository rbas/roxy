use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};

use super::dns_server::DnsServer;
use super::router::{AppState, create_router};
use super::tls::create_tls_acceptor;
use crate::domain::DomainName;
use crate::infrastructure::config::Config;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;

pub struct Server {
    state: Arc<AppState>,
    tls_acceptor: Option<TlsAcceptor>,
    http_port: u16,
    https_port: u16,
    dns_port: u16,
    lan_ip: Ipv4Addr,
}

impl Server {
    pub fn new(config: &Config, paths: &RoxyPaths) -> Result<Self> {
        // Validate config before starting
        config.validate()?;

        let registrations: Vec<_> = config.domains.values().cloned().collect();
        let state = Arc::new(AppState::new(registrations));

        // Get domains with HTTPS enabled
        let https_domains: Vec<DomainName> = config
            .domains
            .values()
            .filter(|d| d.https_enabled)
            .map(|d| d.domain.clone())
            .collect();

        let tls_acceptor = create_tls_acceptor(&https_domains, &paths.certs_dir)?;

        // Get LAN IP for DNS responses (DNS server handles source-based resolution)
        let lan_ip = get_lan_ip();

        Ok(Self {
            state,
            tls_acceptor,
            http_port: config.daemon.http_port,
            https_port: config.daemon.https_port,
            dns_port: config.daemon.dns_port,
            lan_ip,
        })
    }

    pub async fn run(self) -> Result<()> {
        info!(
            http = self.http_port,
            https = self.https_port,
            dns = self.dns_port,
            lan_ip = %self.lan_ip,
            "Roxy daemon starting"
        );

        // Start DNS server with LAN IP (handles source-based IP resolution internally)
        let dns_server = DnsServer::new(self.dns_port, self.lan_ip);
        let dns_handle = tokio::spawn(async move {
            if let Err(e) = dns_server.run().await {
                error!(error = %e, "DNS server error");
            }
        });

        let http_addr = SocketAddr::from(([0, 0, 0, 0], self.http_port));
        let https_addr = SocketAddr::from(([0, 0, 0, 0], self.https_port));

        // Start HTTP server - always serve content (no redirect to HTTPS)
        let http_router = create_router(self.state.clone());

        let http_listener = TcpListener::bind(http_addr).await.context(format!(
            "Failed to bind to port {}. Is another service using it? Try: sudo lsof -i :{}",
            self.http_port, self.http_port
        ))?;

        info!(addr = %http_addr, "HTTP server listening");

        let http_server = tokio::spawn(async move {
            axum::serve(http_listener, http_router)
                .await
                .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))
        });

        // Start HTTPS server if TLS is available
        if let Some(tls_acceptor) = self.tls_acceptor {
            let https_router = create_router(self.state);
            let https_listener = TcpListener::bind(https_addr).await.context(format!(
                "Failed to bind to port {}. Is another service using it? Try: sudo lsof -i :{}",
                self.https_port, self.https_port
            ))?;

            info!(addr = %https_addr, "HTTPS server listening");

            let https_server = tokio::spawn(async move {
                loop {
                    let (stream, _addr) = match https_listener.accept().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            error!(error = %e, "Failed to accept connection");
                            continue;
                        }
                    };

                    let acceptor = tls_acceptor.clone();
                    let router = https_router.clone();

                    tokio::spawn(async move {
                        let stream = match acceptor.accept(stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                warn!(error = %e, "TLS handshake failed");
                                return;
                            }
                        };

                        let io = hyper_util::rt::TokioIo::new(stream);
                        let service =
                            hyper_util::service::TowerToHyperService::new(router.into_service());

                        if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        )
                        .serve_connection_with_upgrades(io, service)
                        .await
                        {
                            error!(error = %e, "Error serving connection");
                        }
                    });
                }
            });

            tokio::select! {
                r = http_server => r??,
                _ = https_server => {},
                _ = dns_handle => {},
            }
        } else {
            warn!(
                "No HTTPS certificates found, running HTTP only. Register a domain with sudo to enable HTTPS."
            );
            tokio::select! {
                r = http_server => r??,
                _ = dns_handle => {},
            }
        }

        Ok(())
    }
}
