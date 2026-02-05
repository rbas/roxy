use std::collections::HashMap;
use std::sync::Arc;

use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::any,
};
use tracing::{debug, info};

use crate::domain::{DomainRegistration, RouteTarget};
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
    let method = request.method().clone();
    let uri = request.uri().clone();

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
            info!(host = %host, "Domain not registered");
            return build_not_registered_response(&host);
        }
    };

    // Match route by path (longest prefix wins)
    let path = uri.path();
    let route = match registration.match_route(path) {
        Some(r) => r,
        None => {
            info!(host = %host, path = %path, "No route found");
            return build_no_route_response(&host, path);
        }
    };

    debug!(
        method = %method,
        host = %host,
        path = %path,
        route = %route.path,
        "Routing request"
    );

    // Route to appropriate backend based on target type
    let response = match &route.target {
        RouteTarget::StaticFiles(dir) => serve_static(dir.clone(), request).await,
        RouteTarget::Proxy(target) => proxy_request(target, request).await,
    };

    info!(
        method = %method,
        host = %host,
        path = %path,
        status = response.status().as_u16(),
        "Request completed"
    );

    response
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
        roxy register {domain} --route "/=3000"<br>
        <small># or with multiple routes:</small><br>
        roxy register {domain} --route "/=3000" --route "/api=3001"
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

fn build_no_route_response(domain: &str, path: &str) -> Response {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>No Route Found - Roxy</title>
    <style>
        body {{ font-family: system-ui, -apple-system, sans-serif; max-width: 600px; margin: 100px auto; padding: 20px; line-height: 1.6; }}
        h1 {{ color: #e74c3c; margin-bottom: 24px; }}
        code {{ background: #f4f4f4; padding: 2px 8px; border-radius: 4px; font-family: 'SF Mono', Menlo, monospace; }}
        .command {{ background: #2d2d2d; color: #f8f8f2; padding: 16px; border-radius: 8px; margin: 16px 0; font-family: 'SF Mono', Menlo, monospace; font-size: 14px; }}
        p {{ color: #444; }}
    </style>
</head>
<body>
    <h1>No Route Found</h1>
    <p>No route matches path <code>{path}</code> on domain <code>{domain}</code>.</p>
    <p>To add a route for this path, run:</p>
    <div class="command">roxy route add {domain} {path} 3000</div>
    <p>Then reload the Roxy daemon:</p>
    <div class="command">roxy reload</div>
</body>
</html>"#,
        domain = domain,
        path = path
    );

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}
