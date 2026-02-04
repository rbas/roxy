use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::any,
};

use crate::domain::{DomainRegistration, Target};
use crate::infrastructure::config::ConfigStore;

use super::proxy::proxy_request;
use super::static_files::serve_static;

/// Shared state for the router
pub struct AppState {
    pub domains: HashMap<String, DomainRegistration>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        let config_store = ConfigStore::new();
        let registrations = config_store.list_domains()?;

        let domains: HashMap<String, DomainRegistration> = registrations
            .into_iter()
            .map(|r| (r.domain.as_str().to_string(), r))
            .collect();

        Ok(Self { domains })
    }

    pub fn get_domain(&self, host: &str) -> Option<&DomainRegistration> {
        // Strip port from host if present
        let domain = host.split(':').next().unwrap_or(host);
        self.domains.get(domain)
    }
}

/// Extract host from request headers
fn get_host(request: &Request) -> Option<String> {
    request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
}

/// Create the main router
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/{*path}", any(handle_request))
        .route("/", any(handle_request))
        .with_state(state)
}

/// Handle all incoming requests
async fn handle_request(State(state): State<Arc<AppState>>, request: Request) -> Response {
    // Extract host from request
    let host = match get_host(&request) {
        Some(h) => h,
        None => {
            return (StatusCode::BAD_REQUEST, "Missing Host header").into_response();
        }
    };

    // Look up the domain
    let registration = match state.get_domain(&host) {
        Some(r) => r,
        None => {
            return build_not_registered_response(&host);
        }
    };

    // Route based on target type
    match &registration.target {
        Target::Path(path) => serve_static(path.clone(), request).await,
        Target::Port(port) => proxy_request(*port, request).await,
    }
}

/// Create HTTP router that redirects to HTTPS
pub fn create_http_redirect_router() -> Router {
    Router::new().fallback(redirect_to_https)
}

async fn redirect_to_https(request: Request) -> impl IntoResponse {
    let host = get_host(&request).unwrap_or_else(|| "localhost".to_string());
    let path = request.uri().path();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    // Strip port from host if present
    let domain = host.split(':').next().unwrap_or(&host);
    let https_url = format!("https://{}{}{}", domain, path, query);

    Redirect::permanent(&https_url)
}

fn build_not_registered_response(domain: &str) -> Response {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Domain Not Registered - Roxy</title>
    <style>
        body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 600px; margin: 100px auto; padding: 20px; line-height: 1.6; }}
        h1 {{ color: #e74c3c; margin-bottom: 24px; }}
        code {{ background: #f4f4f4; padding: 2px 8px; border-radius: 4px; font-family: 'SF Mono', Menlo, monospace; }}
        .command {{ background: #2d2d2d; color: #f8f8f2; padding: 16px; border-radius: 8px; margin: 16px 0; font-family: 'SF Mono', Menlo, monospace; font-size: 14px; }}
        .command small {{ color: #888; }}
        p {{ color: #444; }}
    </style>
</head>
<body>
    <h1>Domain Not Registered</h1>
    <p>The domain <code>{domain}</code> is not registered with Roxy.</p>
    <p>To register this domain, run:</p>
    <div class="command">
        roxy register {domain} --port 3000<br>
        <small># or for static files:</small><br>
        roxy register {domain} --path /path/to/static/files
    </div>
    <p>Then restart the Roxy daemon:</p>
    <div class="command">roxy restart</div>
</body>
</html>"#,
        domain = domain
    );

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}
