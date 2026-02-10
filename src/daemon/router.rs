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

use super::embedded_assets;
use super::proxy::proxy_request;
use super::static_files::serve_static;
use super::theme;

/// Shared state for the router
pub struct AppState {
    pub domains: HashMap<String, DomainRegistration>,
}

impl AppState {
    pub fn new(registrations: Vec<DomainRegistration>) -> Self {
        let domains: HashMap<String, DomainRegistration> = registrations
            .into_iter()
            .map(|r| (r.domain.as_str().to_string(), r))
            .collect();

        Self { domains }
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
        RouteTarget::StaticFiles(dir) => {
            serve_static(route.path.as_str(), dir.clone(), request).await
        }
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
    let domain = theme::html_escape(domain);
    let image_data_uri = embedded_assets::roxy_error_data_uri();

    let mut body = String::new();
    body.push_str("<div class=\"error-container\">\n");
    body.push_str("<div class=\"error-image\">\n");
    body.push_str("<img src=\"");
    body.push_str(image_data_uri);
    body.push_str("\" alt=\"Server Error - Roxy Fox\" ");
    body.push_str("width=\"300\" height=\"225\">\n");
    body.push_str("</div>\n");
    body.push_str("<div class=\"error-card\">\n");
    body.push_str("<h1 class=\"error-title\">Domain Not Registered</h1>\n");
    body.push_str("<p class=\"error-message\">The domain <code>");
    body.push_str(&domain);
    body.push_str("</code> is not registered with Roxy.</p>\n");
    body.push_str("<div class=\"help-section\">\n");
    body.push_str("<p class=\"help-label\">To register this domain, run:</p>\n");
    body.push_str("<div class=\"command\">");
    body.push_str("roxy register ");
    body.push_str(&domain);
    body.push_str(" --route \"/=3000\"<br>");
    body.push_str("<span class=\"comment\"># or with multiple routes:</span><br>");
    body.push_str("roxy register ");
    body.push_str(&domain);
    body.push_str(" --route \"/=3000\" --route \"/api=3001\"");
    body.push_str("</div>\n");
    body.push_str("<p class=\"help-label\">Then restart the Roxy daemon:</p>\n");
    body.push_str("<div class=\"command\">roxy restart</div>\n");
    body.push_str("</div></div></div>");

    let html = theme::render_page("Domain Not Registered", &body, ERROR_CSS, "");

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}

fn build_no_route_response(domain: &str, path: &str) -> Response {
    let domain = theme::html_escape(domain);
    let path = theme::html_escape(path);
    let image_data_uri = embedded_assets::roxy_404_data_uri();

    let mut body = String::new();
    body.push_str("<div class=\"error-container\">\n");
    body.push_str("<div class=\"error-image\">\n");
    body.push_str("<img src=\"");
    body.push_str(image_data_uri);
    body.push_str("\" alt=\"404 - Roxy Fox\" ");
    body.push_str("width=\"300\" height=\"225\">\n");
    body.push_str("</div>\n");
    body.push_str("<div class=\"error-card\">\n");
    body.push_str("<h1 class=\"error-title\">No Route Found</h1>\n");
    body.push_str("<p class=\"error-message\">No route matches path <code>");
    body.push_str(&path);
    body.push_str("</code> on domain <code>");
    body.push_str(&domain);
    body.push_str("</code>.</p>\n");
    body.push_str("<div class=\"help-section\">\n");
    body.push_str("<p class=\"help-label\">To add a route for this path, run:</p>\n");
    body.push_str("<div class=\"command\">roxy route add ");
    body.push_str(&domain);
    body.push(' ');
    body.push_str(&path);
    body.push_str(" 3000</div>\n");
    body.push_str("<p class=\"help-label\">Then reload the Roxy daemon:</p>\n");
    body.push_str("<div class=\"command\">roxy reload</div>\n");
    body.push_str("</div></div></div>");

    let html = theme::render_page("No Route Found", &body, ERROR_CSS, "");

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}

const ERROR_CSS: &str = "\
.error-container{\
    display:flex;flex-direction:column;align-items:center;\
    gap:28px;max-width:700px;margin:40px auto;\
}\
.error-image{\
    animation:fadeInDown .6s ease-out;\
}\
.error-image img{\
    width:100%;max-width:300px;height:auto;\
    border-radius:12px;\
    box-shadow:0 8px 24px rgba(156,180,212,.25);\
}\
.error-card{\
    background:var(--card-bg);border-radius:12px;\
    border:1px solid var(--border);padding:40px;\
    box-shadow:0 4px 16px rgba(0,0,0,.04);\
    text-align:center;width:100%;\
    animation:fadeInUp .6s ease-out;\
}\
.error-title{\
    color:var(--fox-orange);font-size:1.7em;margin-bottom:16px;\
    font-weight:700;\
}\
.error-message{margin-bottom:20px;font-size:1.05em;line-height:1.6}\
.help-section{margin-top:24px;text-align:left}\
.help-label{\
    font-size:.9em;color:var(--text-light);margin-bottom:8px;\
    font-weight:500;text-transform:uppercase;letter-spacing:.03em;\
}\
.command{\
    background:linear-gradient(135deg,#2D2520 0%,#3D3530 100%);\
    color:#F8F0E8;padding:16px 20px;\
    border-radius:10px;margin:12px 0 20px 0;\
    font-family:'SF Mono',Monaco,'Cascadia Code',Menlo,Consolas,monospace;\
    font-size:.88em;line-height:1.8;\
    border:1px solid #4D4540;\
    box-shadow:0 4px 12px rgba(0,0,0,.15);\
}\
.comment{color:#A89C95;font-size:.95em}\
@keyframes fadeInDown{from{opacity:0;transform:translateY(-20px)}to{opacity:1;transform:translateY(0)}}\
@keyframes fadeInUp{from{opacity:0;transform:translateY(20px)}to{opacity:1;transform:translateY(0)}}\
@media(max-width:600px){.error-image img{max-width:240px}.error-card{padding:28px}}\
";
