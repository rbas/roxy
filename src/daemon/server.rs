use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::dns_server::DnsServer;
use super::router::{AppState, create_router};
use super::tls::create_tls_acceptor;
use crate::domain::DomainName;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::logging::LogFile;
use crate::infrastructure::network::get_lan_ip;

pub struct Server {
    state: Arc<AppState>,
    tls_acceptor: Option<TlsAcceptor>,
    http_port: u16,
    https_port: u16,
    dns_port: u16,
    lan_ip: Ipv4Addr,
}

impl Server {
    pub fn new() -> Result<Self> {
        let config_store = ConfigStore::new();
        let config = config_store.load()?;

        // Validate config before starting
        config.validate()?;

        let state = Arc::new(AppState::new()?);

        // Get domains with HTTPS enabled
        let https_domains: Vec<DomainName> = config
            .domains
            .values()
            .filter(|d| d.https_enabled)
            .map(|d| d.domain.clone())
            .collect();

        let tls_acceptor = create_tls_acceptor(&https_domains)?;

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
        let log = LogFile::new();
        let _ = log.log("Daemon starting...");

        // Log LAN IP for debugging
        let _ = log.log(&format!(
            "DNS will use source-based resolution (LAN IP: {})",
            self.lan_ip
        ));
        println!(
            "DNS will use source-based resolution (LAN IP: {})",
            self.lan_ip
        );

        // Start DNS server with LAN IP (handles source-based IP resolution internally)
        let dns_server = DnsServer::new(self.dns_port, self.lan_ip);
        let dns_handle = tokio::spawn(async move {
            if let Err(e) = dns_server.run().await {
                eprintln!("DNS server error: {}", e);
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

        let _ = log.log(&format!("HTTP server listening on {}", http_addr));
        println!("HTTP server listening on {}", http_addr);

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

            let _ = log.log(&format!("HTTPS server listening on {}", https_addr));
            println!("HTTPS server listening on {}", https_addr);

            let https_server = tokio::spawn(async move {
                loop {
                    let (stream, _addr) = match https_listener.accept().await {
                        Ok(conn) => conn,
                        Err(e) => {
                            eprintln!("Failed to accept connection: {}", e);
                            continue;
                        }
                    };

                    let acceptor = tls_acceptor.clone();
                    let router = https_router.clone();

                    tokio::spawn(async move {
                        let stream = match acceptor.accept(stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("TLS handshake failed: {}", e);
                                return;
                            }
                        };

                        let io = hyper_util::rt::TokioIo::new(stream);
                        let service =
                            hyper_util::service::TowerToHyperService::new(router.into_service());

                        if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        )
                        .serve_connection(io, service)
                        .await
                        {
                            eprintln!("Error serving connection: {}", e);
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
            println!("Warning: No HTTPS certificates found. Running HTTP only.");
            println!("Register a domain with sudo to enable HTTPS.");
            tokio::select! {
                r = http_server => r??,
                _ = dns_handle => {},
            }
        }

        Ok(())
    }
}
