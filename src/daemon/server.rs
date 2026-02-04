use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::router::{AppState, create_http_redirect_router, create_router};
use super::tls::create_tls_acceptor;
use crate::domain::DomainName;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::logging::LogFile;

pub struct Server {
    state: Arc<AppState>,
    tls_acceptor: Option<TlsAcceptor>,
}

impl Server {
    pub fn new() -> Result<Self> {
        let state = Arc::new(AppState::new()?);

        // Get domains with HTTPS enabled
        let config_store = ConfigStore::new();
        let https_domains: Vec<DomainName> = config_store
            .list_domains()?
            .into_iter()
            .filter(|d| d.https_enabled)
            .map(|d| d.domain)
            .collect();

        let tls_acceptor = create_tls_acceptor(&https_domains)?;

        Ok(Self {
            state,
            tls_acceptor,
        })
    }

    pub async fn run(self) -> Result<()> {
        let log = LogFile::new();
        let _ = log.log("Daemon starting...");

        let http_addr = SocketAddr::from(([0, 0, 0, 0], 80));
        let https_addr = SocketAddr::from(([0, 0, 0, 0], 443));

        // Start HTTP server (redirects to HTTPS if TLS available, otherwise serves directly)
        let http_router = if self.tls_acceptor.is_some() {
            create_http_redirect_router()
        } else {
            create_router(self.state.clone())
        };

        let http_listener = TcpListener::bind(http_addr).await.context(
            "Failed to bind to port 80. Is another service using it? Try: sudo lsof -i :80",
        )?;

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
            let https_listener = TcpListener::bind(https_addr).await.context(
                "Failed to bind to port 443. Is another service using it? Try: sudo lsof -i :443",
            )?;

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
            }
        } else {
            println!("Warning: No HTTPS certificates found. Running HTTP only.");
            println!("Register a domain with sudo to enable HTTPS.");
            http_server.await??;
        }

        Ok(())
    }
}
