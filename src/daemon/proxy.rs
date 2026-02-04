use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::domain::value_objects::Port;

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
fn build_upgrade_request(request: &Request, port: Port) -> String {
    let path = request.uri().path();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();

    let mut req = format!(
        "GET {}{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n",
        path, query, port
    );

    for (name, value) in request.headers() {
        if name != header::HOST
            && let Ok(v) = value.to_str()
        {
            req.push_str(&format!("{}: {}\r\n", name, v));
        }
    }
    req.push_str("\r\n");
    req
}

/// Proxy a WebSocket connection
async fn proxy_websocket(port: Port, request: Request) -> Response {
    // Connect to backend
    let backend_addr = format!("127.0.0.1:{}", port);
    let mut backend = match TcpStream::connect(&backend_addr).await {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::BAD_GATEWAY,
                format!("Cannot connect to service on port {}", port),
            )
                .into_response();
        }
    };

    // Build and send the upgrade request to backend
    let upgrade_request = build_upgrade_request(&request, port);

    if let Err(e) = backend.write_all(upgrade_request.as_bytes()).await {
        return (
            StatusCode::BAD_GATEWAY,
            format!("Backend write error: {}", e),
        )
            .into_response();
    }

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

                tokio::select! {
                    _ = client_to_backend => {},
                    _ = backend_to_client => {},
                }
            }
            Err(e) => {
                eprintln!("WebSocket upgrade error: {}", e);
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

/// Proxy a request to a local port (supports HTTP/1.1, HTTP/2, and WebSocket)
pub async fn proxy_request(port: Port, request: Request) -> Response {
    // Check for WebSocket upgrade
    if is_websocket_upgrade(&request) {
        return proxy_websocket(port, request).await;
    }

    // Regular HTTP proxy
    let mut connector = HttpConnector::new();
    connector.set_nodelay(true);

    let client = Client::builder(TokioExecutor::new()).build(connector);

    // Rewrite the URI to target localhost:port
    let path = request.uri().path();
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let uri_string = format!("http://127.0.0.1:{}{}{}", port, path, query);

    let uri: Uri = match uri_string.parse() {
        Ok(u) => u,
        Err(e) => {
            return (StatusCode::BAD_REQUEST, format!("Invalid URI: {}", e)).into_response();
        }
    };

    let mut request = request;
    *request.uri_mut() = uri;

    // Remove host header (will be set by client)
    request.headers_mut().remove("host");

    // Forward the request
    match client.request(request).await {
        Ok(response) => {
            let (parts, body) = response.into_parts();
            Response::from_parts(parts, Body::new(body))
        }
        Err(e) => {
            // Check if it's a connection error (service not running)
            let error_msg = e.to_string();
            if error_msg.contains("Connection refused") {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Service not running on port {}", port),
                )
                    .into_response()
            } else {
                (StatusCode::BAD_GATEWAY, format!("Proxy error: {}", e)).into_response()
            }
        }
    }
}
