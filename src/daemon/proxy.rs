use std::net::IpAddr;
use std::time::Instant;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode, Uri, header, header::HeaderName, header::HeaderValue},
    response::{IntoResponse, Response},
};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use crate::domain::ProxyTarget;

/// Scheme of the original client request (injected by server layers).
#[derive(Clone, Copy)]
pub struct Scheme(pub &'static str);

/// Client IP address (injected by server layers).
#[derive(Clone, Copy)]
pub struct ClientAddr(pub IpAddr);

/// Check if request is a WebSocket upgrade
fn is_websocket_upgrade(request: &Request) -> bool {
    request
        .headers()
        .get(header::UPGRADE)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

/// Build HTTP upgrade request string to send to backend
fn build_upgrade_request(
    request: &Request,
    target: &ProxyTarget,
    host: &str,
    scheme: &str,
    client_ip: Option<IpAddr>,
) -> String {
    let path = request.uri().path();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let mut req = format!(
        "GET {}{} HTTP/1.1\r\nHost: {}:{}\r\n",
        path,
        query,
        target.host(),
        target.port()
    );

    // Forwarding headers
    req.push_str(&format!("X-Forwarded-Host: {}\r\n", host));
    req.push_str(&format!("X-Forwarded-Proto: {}\r\n", scheme));
    if let Some(ip) = client_ip {
        let xff = match request
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            Some(existing) => format!("{}, {}", existing, ip),
            None => ip.to_string(),
        };
        req.push_str(&format!("X-Forwarded-For: {}\r\n", xff));
    }

    // Copy remaining headers, skipping Host and forwarding headers we already set
    for (name, value) in request.headers() {
        if name == header::HOST
            || name.as_str() == "x-forwarded-host"
            || name.as_str() == "x-forwarded-proto"
            || name.as_str() == "x-forwarded-for"
        {
            continue;
        }
        if let Ok(v) = value.to_str() {
            req.push_str(&format!("{}: {}\r\n", name, v));
        }
    }
    req.push_str("\r\n");
    req
}

/// Proxy a WebSocket connection
async fn proxy_websocket(
    target: &ProxyTarget,
    request: Request,
    host: &str,
    scheme: &str,
    client_ip: Option<IpAddr>,
) -> Response {
    // Connect to backend
    let backend_addr = format!("{}:{}", target.host(), target.port());
    debug!(target = %target, "Connecting to backend for WebSocket");
    let mut backend = match TcpStream::connect(&backend_addr).await {
        Ok(s) => s,
        Err(_) => {
            warn!(target = %target, "WebSocket backend connection failed");
            return (
                StatusCode::BAD_GATEWAY,
                format!("Cannot connect to service at {}", target),
            )
                .into_response();
        }
    };
    let start_time = Instant::now();

    // Build and send the upgrade request to backend
    let upgrade_request = build_upgrade_request(&request, target, host, scheme, client_ip);

    if let Err(e) = backend.write_all(upgrade_request.as_bytes()).await {
        return (
            StatusCode::BAD_GATEWAY,
            format!("Backend write error: {}", e),
        )
            .into_response();
    }
    debug!(target = %target, "WebSocket upgrade request sent to backend");

    // Read backend response (101 Switching Protocols expected)
    let mut response_buf = vec![0u8; 4096];
    let n = match backend.read(&mut response_buf).await {
        Ok(n) => n,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Backend read error: {}", e),
            )
                .into_response();
        }
    };

    // Verify we got 101 Switching Protocols
    let response_str = String::from_utf8_lossy(&response_buf[..n]);
    if !response_str.contains("101") {
        warn!(target = %target, "Backend rejected WebSocket upgrade");
        return (
            StatusCode::BAD_GATEWAY,
            "Backend rejected WebSocket upgrade",
        )
            .into_response();
    }

    // Extract Sec-WebSocket-Accept from backend response
    let accept_key = response_str
        .lines()
        .find(|line| line.to_lowercase().starts_with("sec-websocket-accept:"))
        .and_then(|line| line.split(':').nth(1))
        .map(|s| s.trim().to_string());

    // Get the OnUpgrade handle from the request
    let on_upgrade = hyper::upgrade::on(request);

    info!(target = %target, "WebSocket connection established");

    // Clone target for the spawned task
    let target_str = target.to_string();

    // Spawn task to handle the bidirectional copy after upgrade
    tokio::spawn(async move {
        match on_upgrade.await {
            Ok(upgraded) => {
                let client = hyper_util::rt::TokioIo::new(upgraded);
                let (mut client_read, mut client_write) = tokio::io::split(client);
                let (mut backend_read, mut backend_write) = backend.into_split();

                let client_to_backend = async {
                    let mut buf = [0u8; 8192];
                    loop {
                        let n = match client_read.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        if backend_write.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                };

                let backend_to_client = async {
                    let mut buf = [0u8; 8192];
                    loop {
                        let n = match backend_read.read(&mut buf).await {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        if client_write.write_all(&buf[..n]).await.is_err() {
                            break;
                        }
                    }
                };

                let closed_by = tokio::select! {
                    _ = client_to_backend => "client",
                    _ = backend_to_client => "backend",
                };

                let duration = start_time.elapsed();
                info!(
                    target = %target_str,
                    duration_ms = duration.as_millis() as u64,
                    "WebSocket connection closed"
                );
                debug!(
                    target = %target_str,
                    closed_by = closed_by,
                    "WebSocket close details"
                );
            }
            Err(e) => {
                warn!(target = %target_str, error = %e, "WebSocket upgrade failed");
            }
        }
    });

    // Build 101 Switching Protocols response for client
    let mut builder = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "Upgrade");

    if let Some(key) = accept_key {
        builder = builder.header("Sec-WebSocket-Accept", key);
    }

    builder.body(Body::empty()).unwrap()
}

/// Set `X-Forwarded-For`, `X-Forwarded-Proto`, and `X-Forwarded-Host` headers.
fn set_forwarding_headers(
    headers: &mut HeaderMap,
    host: &str,
    scheme: &str,
    client_ip: Option<IpAddr>,
) {
    if let Ok(value) = HeaderValue::from_str(host) {
        headers.insert("x-forwarded-host", value);
    }

    if let Ok(value) = HeaderValue::from_str(scheme) {
        headers.insert("x-forwarded-proto", value);
    }

    let xff = if let Some(ip) = client_ip {
        let value = match headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            Some(existing) => format!("{}, {}", existing, ip),
            None => ip.to_string(),
        };
        if let Ok(hv) = HeaderValue::from_str(&value) {
            headers.insert("x-forwarded-for", hv);
        }
        value
    } else {
        String::new()
    };

    debug!(
        x_forwarded_host = %host,
        x_forwarded_proto = %scheme,
        x_forwarded_for = %xff,
        "Forwarding headers set"
    );
}

/// Remove hop-by-hop headers that must not be forwarded (RFC 2616 §13.5.1).
///
/// Also strips any extra headers listed in the `Connection` header value.
fn strip_hop_by_hop_headers(headers: &mut HeaderMap) {
    // Collect headers named in the Connection value (e.g. "Connection: X-Custom, keep-alive").
    let connection_headers: Vec<HeaderName> = headers
        .get(header::CONNECTION)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            s.split(',')
                .filter_map(|token| HeaderName::from_bytes(token.trim().as_bytes()).ok())
                .collect()
        })
        .unwrap_or_default();

    headers.remove(header::CONNECTION);
    headers.remove("keep-alive");
    headers.remove(header::PROXY_AUTHENTICATE);
    headers.remove(header::PROXY_AUTHORIZATION);
    headers.remove(header::TE);
    headers.remove(header::TRAILER);
    headers.remove(header::TRANSFER_ENCODING);

    for name in connection_headers {
        headers.remove(&name);
    }
}

/// Proxy a request to a backend (supports HTTP/1.1, HTTP/2, and WebSocket)
pub async fn proxy_request(
    target: &ProxyTarget,
    request: Request,
    host: &str,
    scheme: &str,
    client_ip: Option<IpAddr>,
) -> Response {
    // Check for WebSocket upgrade
    if is_websocket_upgrade(&request) {
        debug!(target = %target, "Proxying WebSocket request");
        return proxy_websocket(target, request, host, scheme, client_ip).await;
    }

    debug!(target = %target, "Proxying HTTP request");

    // Regular HTTP proxy
    let mut connector = HttpConnector::new();
    connector.set_nodelay(true);

    let client = Client::builder(TokioExecutor::new()).build(connector);

    // Rewrite the URI to target the backend
    let path = request.uri().path();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let uri_string = format!(
        "http://{}:{}{}{}",
        target.host(),
        target.port(),
        path,
        query
    );

    let uri: Uri = match uri_string.parse() {
        Ok(u) => u,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Invalid URI: {}", e)).into_response();
        }
    };

    let mut request = request;
    *request.uri_mut() = uri;

    // Set forwarding headers before removing Host
    set_forwarding_headers(request.headers_mut(), host, scheme, client_ip);

    // Remove original Host header (hyper client sets it for the target)
    request.headers_mut().remove(header::HOST);

    // Strip hop-by-hop headers
    strip_hop_by_hop_headers(request.headers_mut());

    // Forward the request
    match client.request(request).await {
        Ok(response) => {
            debug!(target = %target, status = %response.status(), "Proxy response");
            let (mut parts, body) = response.into_parts();
            strip_hop_by_hop_headers(&mut parts.headers);
            Response::from_parts(parts, Body::new(body))
        }
        Err(e) => {
            // Check if it's a connection error (service not running)
            let error_msg = e.to_string();
            if error_msg.contains("Connection refused") {
                warn!(target = %target, "Service not running");
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Service not running at {}", target),
                )
                    .into_response()
            } else {
                warn!(target = %target, error = %e, "Proxy failed");
                (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)).into_response()
            }
        }
    }
}
