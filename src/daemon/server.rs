use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use axum::{
    Extension, extract::ConnectInfo, extract::Request, middleware::Next, response::Response,
};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_rustls::TlsAcceptor;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use std::path::PathBuf;

use super::dns_server::DnsServer;
use super::proxy::{ClientAddr, Scheme};
use super::router::{RuntimeState, SharedState, create_router};
use super::tls::create_tls_acceptor;
use crate::application::ports::RegistrationProvider;
use crate::domain::DomainRegistration;
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
    state: SharedState,
    reload_rx: mpsc::Receiver<Vec<DomainRegistration>>,
    reload_tx: mpsc::Sender<Vec<DomainRegistration>>,
    reload_provider: Arc<dyn RegistrationProvider>,
    socket_path: PathBuf,
    tls_acceptor: Option<TlsAcceptor>,
    http_port: u16,
    https_port: u16,
    dns_port: u16,
    lan_ip: Ipv4Addr,
}

impl Server {
    pub fn new(
        config: &Config,
        paths: &RoxyPaths,
        reload_rx: mpsc::Receiver<Vec<DomainRegistration>>,
        reload_tx: mpsc::Sender<Vec<DomainRegistration>>,
        reload_provider: Arc<dyn RegistrationProvider>,
    ) -> Result<Self> {
        // Validate config before starting
        config.validate()?;

        let registrations = config.registrations();

        // Collect patterns for domains with HTTPS enabled
        let https_patterns: Vec<_> = registrations
            .iter()
            .filter(|d| d.is_https_enabled())
            .map(|d| d.pattern().clone())
            .collect();

        let state: SharedState = Arc::new(ArcSwap::from_pointee(RuntimeState::new(registrations)));

        let tls_acceptor = create_tls_acceptor(&https_patterns, &paths.certs_dir, &paths.data_dir)?;

        // Get LAN IP for DNS responses (DNS server handles source-based resolution)
        let lan_ip = get_lan_ip();

        Ok(Self {
            state,
            reload_rx,
            reload_tx,
            reload_provider,
            socket_path: paths.socket_path.clone(),
            tls_acceptor,
            http_port: config.daemon.http_port,
            https_port: config.daemon.https_port,
            dns_port: config.daemon.dns_port,
            lan_ip,
        })
    }

    pub async fn run(self, cancel: CancellationToken) -> Result<()> {
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

        // Spawn reload listener — swaps RuntimeState when new registrations arrive
        let state_for_reload = self.state.clone();
        let cancel_reload = cancel.clone();
        let mut reload_rx = self.reload_rx;
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_reload.cancelled() => break,
                    msg = reload_rx.recv() => match msg {
                        Some(regs) => {
                            let count = regs.len();
                            let new = Arc::new(RuntimeState::new(regs));
                            state_for_reload.store(new);
                            info!(count, "Runtime state reloaded");
                        }
                        None => break,
                    }
                }
            }
        });

        // Spawn management socket for status/reload/list commands
        let mgmt_cancel = cancel.clone();
        tokio::spawn({
            let state = self.state.clone();
            let reload_tx = self.reload_tx.clone();
            let reload_provider = self.reload_provider.clone();
            let socket_path = self.socket_path.clone();
            async move {
                if let Err(e) = super::mgmt_socket::serve(
                    socket_path,
                    state,
                    reload_provider,
                    reload_tx,
                    mgmt_cancel,
                )
                .await
                {
                    error!(error = %e, "Management socket error");
                }
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

        let http_server = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                axum::serve(
                    http_listener,
                    http_router.into_make_service_with_connect_info::<SocketAddr>(),
                )
                .with_graceful_shutdown(cancel.cancelled_owned())
                .await
                .map_err(|e| anyhow::anyhow!("HTTP server error: {}", e))
            }
        });

        // Start HTTPS server if TLS is available
        if let Some(tls_acceptor) = self.tls_acceptor {
            let https_router = create_router(self.state).layer(Extension(Scheme::Https));
            let https_listener = TcpListener::bind(https_addr).await.context(format!(
                "Failed to bind to port {}. Is another service using it? Try: sudo lsof -i :{}",
                self.https_port, self.https_port
            ))?;

            info!(addr = %https_addr, "HTTPS server listening");

            let https_server = tokio::spawn({
                let cancel = cancel.clone();
                async move {
                    loop {
                        let (stream, addr) = tokio::select! {
                            _ = cancel.cancelled() => break,
                            result = https_listener.accept() => match result {
                                Ok(conn) => conn,
                                Err(e) => {
                                    error!(error = %e, "Failed to accept connection");
                                    continue;
                                }
                            },
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
                            let service = hyper_util::service::TowerToHyperService::new(
                                router.into_service(),
                            );

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
                }
            });

            tokio::select! {
                r = http_server => r??,
                r = https_server => {
                    if let Err(e) = r {
                        error!(error = %e, "HTTPS server task failed");
                        anyhow::bail!("HTTPS server task failed: {e}");
                    }
                },
                r = dns_handle => {
                    if let Err(e) = r {
                        error!(error = %e, "DNS server task failed");
                        anyhow::bail!("DNS server task failed: {e}");
                    }
                },
            }
        } else {
            warn!(
                "No HTTPS certificates found, running HTTP only. \
                 Register a domain with sudo to enable HTTPS."
            );
            tokio::select! {
                r = http_server => r??,
                r = dns_handle => {
                    if let Err(e) = r {
                        error!(error = %e, "DNS server task failed");
                        anyhow::bail!("DNS server task failed: {e}");
                    }
                },
            }
        }

        Ok(())
    }
}
