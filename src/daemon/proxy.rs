use axum::{
    body::Body,
    extract::Request,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use crate::domain::value_objects::Port;

/// Proxy a request to a local port
pub async fn proxy_request(port: Port, mut request: Request) -> Response {
    let client = Client::builder(TokioExecutor::new()).build_http();

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
