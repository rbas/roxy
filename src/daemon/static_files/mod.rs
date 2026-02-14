mod breadcrumb;
mod directory;
mod path_utils;
mod styles;

use std::path::PathBuf;

use axum::{
    extract::Request,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tokio::task;
use tower::ServiceExt;
use tower_http::services::ServeDir;

use super::embedded_assets;
use super::theme;
use directory::try_directory_listing;
use path_utils::resolve_path;
use styles::NOT_FOUND_CSS;

/// Serve static files from a directory.
///
/// If the request path maps to a directory without an `index.html`,
/// renders an HTML directory listing with sortable columns.
pub async fn serve_static(route_prefix: &str, root: PathBuf, request: Request) -> Response {
    let original_path = request.uri().path().to_string();
    let method = request.method().clone();
    let query = request.uri().query().map(|q| q.to_string());

    let stripped_path = strip_route_prefix(&original_path, route_prefix);

    // Preserve the typical "directory path should end with '/'" behavior for mount roots.
    if (method == axum::http::Method::GET || method == axum::http::Method::HEAD)
        && route_prefix != "/"
        && original_path == route_prefix
    {
        let location = if let Some(query) = query {
            format!("{route_prefix}/?{query}")
        } else {
            format!("{route_prefix}/")
        };
        return redirect_to(&location);
    }

    let mut request_for_service = request;
    rewrite_request_uri_path(&mut request_for_service, &stripped_path);

    let service = ServeDir::new(&root).append_index_html_on_directories(true);

    // Non-GET/HEAD methods should keep ServeDir's behavior (typically 405).
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return match service.oneshot(request_for_service).await {
            Ok(mut response) => {
                rewrite_redirect_location_to_include_mount_prefix(route_prefix, &mut response);
                response.into_response()
            }
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
        };
    }

    // Security check: ensure the request resolves within the configured root.
    let root_for_resolve = root.clone();
    let stripped_for_resolve = stripped_path.clone();
    let resolved: Option<PathBuf> =
        task::spawn_blocking(move || resolve_path(&root_for_resolve, &stripped_for_resolve))
            .await
            .unwrap_or_default();

    let Some(resolved) = resolved else {
        return build_not_found_response(&original_path);
    };

    match service.oneshot(request_for_service).await {
        Ok(mut response) => {
            if response.status() == StatusCode::NOT_FOUND {
                if let Some(listing) =
                    try_directory_listing(route_prefix, &original_path, resolved.clone()).await
                {
                    return listing;
                }
                return build_not_found_response(&original_path);
            }

            rewrite_redirect_location_to_include_mount_prefix(route_prefix, &mut response);
            response.into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response(),
    }
}

fn strip_route_prefix(uri_path: &str, route_prefix: &str) -> String {
    if route_prefix == "/" {
        return uri_path.to_string();
    }

    if uri_path == route_prefix {
        return "/".to_string();
    }

    if let Some(rest) = uri_path.strip_prefix(route_prefix) {
        if rest.is_empty() {
            "/".to_string()
        } else {
            rest.to_string()
        }
    } else {
        // Shouldn't happen since routing already matched, but keep behavior safe.
        uri_path.to_string()
    }
}

fn rewrite_request_uri_path(request: &mut Request, new_path: &str) {
    let uri_str = if let Some(query) = request.uri().query() {
        format!("{new_path}?{query}")
    } else {
        new_path.to_string()
    };

    if let Ok(uri) = uri_str.parse::<Uri>() {
        *request.uri_mut() = uri;
    }
}

fn redirect_to(location: &str) -> Response {
    Response::builder()
        .status(StatusCode::TEMPORARY_REDIRECT)
        .header(header::LOCATION, location)
        .body(axum::body::Body::empty())
        .unwrap()
}

/// `ServeDir` generates redirects based on the request URI it sees.
///
/// Since we strip the mount prefix before calling `ServeDir`, redirects such as
/// "add a trailing slash to directory paths" would otherwise point outside the
/// mounted route. This rewrites absolute-path `Location` headers to include the
/// mount prefix.
fn rewrite_redirect_location_to_include_mount_prefix<ResBody>(
    mount_prefix: &str,
    response: &mut axum::http::Response<ResBody>,
) {
    if mount_prefix == "/" || !response.status().is_redirection() {
        return;
    }

    let Some(location) = response.headers().get(header::LOCATION) else {
        return;
    };
    let Ok(location_str) = location.to_str() else {
        return;
    };

    if !location_str.starts_with('/') {
        return;
    }

    let new_location = format!("{mount_prefix}{location_str}");
    if let Ok(new_location) = axum::http::HeaderValue::from_str(&new_location) {
        response
            .headers_mut()
            .insert(header::LOCATION, new_location);
    }
}

/// Build a themed 404 page for files that don't exist.
fn build_not_found_response(uri_path: &str) -> Response {
    let path = theme::html_escape(uri_path);
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
    body.push_str("<h1 class=\"error-title\">File Not Found</h1>\n");
    body.push_str("<p class=\"error-message\">The path <code>");
    body.push_str(&path);
    body.push_str("</code> does not exist.</p>\n");
    body.push_str("<p class=\"error-hint\">Check the path and try again, or navigate back to the directory listing.</p>\n");
    body.push_str("</div></div>");

    let html = theme::render_page("File Not Found", &body, NOT_FOUND_CSS, "");

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_redirect_location_to_include_mount_prefix() {
        let mut response = Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(header::LOCATION, "/docs/")
            .body(axum::body::Body::empty())
            .unwrap();

        rewrite_redirect_location_to_include_mount_prefix("/static", &mut response);

        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(location, "/static/docs/");
    }

    #[test]
    fn test_rewrite_redirect_location_to_include_mount_prefix_noop_for_root() {
        let mut response = Response::builder()
            .status(StatusCode::TEMPORARY_REDIRECT)
            .header(header::LOCATION, "/docs/")
            .body(axum::body::Body::empty())
            .unwrap();

        rewrite_redirect_location_to_include_mount_prefix("/", &mut response);

        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(location, "/docs/");
    }
}
