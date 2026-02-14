use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Extension, extract::ConnectInfo, extract::Request, middleware::Next, response::Response,
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tracing::{error, info, warn};

use super::dns_server::DnsServer;
use super::proxy::{ClientAddr, Scheme};
use super::router::{AppState, create_router};
use super::tls::create_tls_acceptor;
use crate::infrastructure::config::Config;
use crate::infrastructure::network::get_lan_ip;
use crate::infrastructure::paths::RoxyPaths;

/// Middleware that copies the client IP from `ConnectInfo` into a `ClientAddr` extension.
async fn inject_client_addr(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut request: Request,
    next: Next,
) -> Response {
    request.extensions_mut().insert(ClientAddr(addr.ip()));
    next.run(request).await
}

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

        let registrations = config.registrations();

        // Collect patterns for domains with HTTPS enabled
        let https_patterns: Vec<_> = registrations
            .iter()
            .filter(|d| d.is_https_enabled())
            .map(|d| d.pattern().clone())
            .collect();

        let state = Arc::new(AppState::new(registrations));

        let tls_acceptor = create_tls_acceptor(&https_patterns, &paths.certs_dir, &paths.data_dir)?;

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
        let http_router = create_router(self.state.clone())
            .layer(Extension(Scheme::Http))
            .layer(axum::middleware::from_fn(inject_client_addr));

        let http_listener = TcpListener::bind(http_addr).await.context(format!(
            "Failed to bind to port {}. Is another service using it? Try: sudo lsof -i :{}",
            self.http_port, self.http_port
        ))?;

        info!(addr = %http_addr, "HTTP server listening");

        let http_server = tokio::spawn(async move {
            axum::serve(
                http_listener,
                http_router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .await
            .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))
        });

        // Start HTTPS server if TLS is available
        if let Some(tls_acceptor) = self.tls_acceptor {
            let https_router = create_router(self.state).layer(Extension(Scheme::Https));
            let https_listener = TcpListener::bind(https_addr).await.context(format!(
                "Failed to bind to port {}. Is another service using it? Try: sudo lsof -i :{}",
                self.https_port, self.https_port
            ))?;

            info!(addr = %https_addr, "HTTPS server listening");

            let https_server = tokio::spawn(async move {
                loop {
                    let (stream, addr) = match https_listener.accept().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            error!(error = %e, "Failed to accept connection");
                            continue;
                        }
                    };

                    let acceptor = tls_acceptor.clone();
                    // The HTTPS path uses manual TLS accept, so ConnectInfo is not
                    // available. Instead, inject the client IP directly as an Extension
                    // on each accepted connection.
                    let router = https_router.clone().layer(Extension(ClientAddr(addr.ip())));

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
