use std::path::PathBuf;

use axum::{
    extract::Request,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

/// Serve static files from a directory
pub async fn serve_static(root: PathBuf, request: Request) -> Response {
    let service = ServeDir::new(&root).append_index_html_on_directories(true);

    match service.oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}
