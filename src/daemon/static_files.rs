use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use axum::{
    extract::Request,
    http::{StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tower::ServiceExt;
use tower_http::services::ServeDir;

use super::embedded_assets;
use super::theme;
use tokio::task;

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

/// Try to render a directory listing for `resolved` and show it as `display_path`.
async fn try_directory_listing(
    route_prefix: &str,
    display_path: &str,
    resolved: PathBuf,
) -> Option<Response> {
    let entries = task::spawn_blocking(move || read_directory(&resolved))
        .await
        .ok()??;

    let html = render_directory_listing(route_prefix, display_path, &entries);

    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/html; charset=utf-8")
        .body(axum::body::Body::from(html))
        .ok()
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

const NOT_FOUND_CSS: &str = "\
.error-container{\
    display:flex;flex-direction:column;align-items:center;\
    gap:24px;max-width:700px;margin:40px auto;\
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
    border:1px solid var(--border);padding:36px;\
    box-shadow:0 4px 16px rgba(0,0,0,.04);\
    text-align:center;width:100%;\
    animation:fadeInUp .6s ease-out;\
}\
.error-title{\
    color:var(--fox-orange);font-size:1.6em;margin-bottom:16px;\
    font-weight:700;\
}\
.error-message{margin-bottom:12px;font-size:1.05em}\
.error-hint{color:var(--text-light);font-size:.92em;margin-top:8px}\
@keyframes fadeInDown{from{opacity:0;transform:translateY(-20px)}to{opacity:1;transform:translateY(0)}}\
@keyframes fadeInUp{from{opacity:0;transform:translateY(20px)}to{opacity:1;transform:translateY(0)}}\
@media(max-width:600px){.error-image img{max-width:240px}.error-card{padding:28px}}\
";

/// Resolve a URI path to a filesystem path within the root directory.
///
/// Returns `None` if the path doesn't exist or escapes the root
/// (path traversal protection).
fn resolve_path(root: &Path, uri_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(uri_path);
    let relative = decoded.trim_start_matches('/');
    let dir_path = if relative.is_empty() {
        root.to_path_buf()
    } else {
        root.join(relative)
    };

    let canonical = dir_path.canonicalize().ok()?;
    let canonical_root = root.canonicalize().ok()?;

    if !canonical.starts_with(&canonical_root) {
        return None;
    }

    Some(canonical)
}

/// Decode percent-encoded URI path segments.
fn percent_decode(s: &str) -> String {
    let mut result = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            result.push(byte);
            i += 3;
            continue;
        }
        result.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&result).to_string()
}

struct DirEntry {
    name: String,
    is_dir: bool,
    size: u64,
    modified: u64,
}

/// Read a directory and collect entries with metadata.
fn read_directory(path: &Path) -> Option<Vec<DirEntry>> {
    let read_dir = fs::read_dir(path).ok()?;

    let mut entries: Vec<DirEntry> = read_dir
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = metadata.is_dir();
            let size = if is_dir { 0 } else { metadata.len() };
            let modified = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            Some(DirEntry {
                name,
                is_dir,
                size,
                modified,
            })
        })
        .collect();

    // Default: directories first, then case-insensitive alphabetical
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Some(entries)
}

/// Format bytes as a human-readable size string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

// ── Breadcrumb ──────────────────────────────────────────────

/// Build a breadcrumb navigation bar from the URI path.
///
/// The `route_prefix` is used as the root href to keep navigation within
/// the mounted route (e.g., `/static/` for non-root mounts).
fn build_breadcrumb(route_prefix: &str, uri_path: &str) -> String {
    let mut html = String::new();

    // Root link with home icon - use route_prefix to stay within mount
    let root_href = if route_prefix == "/" {
        "/"
    } else {
        route_prefix
    };
    html.push_str("<a href=\"");
    html.push_str(root_href);
    if !root_href.ends_with('/') {
        html.push('/');
    }
    html.push_str("\" title=\"Root\">");
    html.push_str(theme::HOME_ICON);
    html.push_str("</a>");

    let path = uri_path.trim_matches('/');
    if !path.is_empty() {
        let mut href = String::new();
        for segment in path.split('/') {
            if segment.is_empty() {
                continue;
            }
            let decoded_segment = percent_decode(segment);
            html.push_str("<span class=\"sep\">/</span>");
            href.push('/');
            href.push_str(&theme::encode_path_segment(&decoded_segment));
            html.push_str("<a href=\"");
            html.push_str(&href);
            html.push_str("/\">");
            html.push_str(&theme::html_escape(&decoded_segment));
            html.push_str("</a>");
        }
    }

    html
}

// ── Directory Listing Renderer ──────────────────────────────

/// Render the full themed HTML page for a directory listing.
///
/// The `route_prefix` is used to determine the mount root and keep navigation
/// within the mounted route.
fn render_directory_listing(route_prefix: &str, uri_path: &str, entries: &[DirEntry]) -> String {
    let display_path = theme::html_escape(uri_path);
    let breadcrumb = build_breadcrumb(route_prefix, uri_path);

    let mut body = String::with_capacity(4096);

    // Page heading
    body.push_str("<h1 class=\"page-title\">Index of <code>");
    body.push_str(&display_path);
    body.push_str("</code></h1>\n");

    // Breadcrumb navigation
    body.push_str("<nav class=\"breadcrumb\">");
    body.push_str(&breadcrumb);
    body.push_str("</nav>\n");

    // File listing card
    body.push_str("<div class=\"file-card\">\n");
    body.push_str("<table id=\"listing\">\n<thead><tr>");
    body.push_str(
        "<th onclick=\"sort(0)\">Name \
         <span class=\"si active\" id=\"s0\">\u{25B2}</span></th>",
    );
    body.push_str(
        "<th onclick=\"sort(1)\" class=\"col-size\">Size \
         <span class=\"si\" id=\"s1\"></span></th>",
    );
    body.push_str(
        "<th onclick=\"sort(2)\" class=\"col-mod\">Modified \
         <span class=\"si\" id=\"s2\"></span></th>",
    );
    body.push_str("</tr></thead>\n<tbody>\n");

    // Determine the mount root for comparison
    let mount_root = if route_prefix == "/" {
        "/"
    } else {
        // Mount root can be either "/prefix" or "/prefix/"
        route_prefix.trim_end_matches('/')
    };
    let normalized_uri = uri_path.trim_end_matches('/');
    let is_at_mount_root = normalized_uri == mount_root || normalized_uri.is_empty();

    // Parent directory link (unless at mount root)
    if !is_at_mount_root {
        let parent = uri_path.trim_end_matches('/');
        let parent = parent
            .rsplit_once('/')
            .map(|(p, _)| if p.is_empty() { "/" } else { p })
            .unwrap_or("/");

        // Only show parent link if it's not outside our mount
        let parent_normalized = parent.trim_end_matches('/');
        let should_show_parent = if route_prefix == "/" {
            true
        } else {
            parent_normalized.starts_with(mount_root) || parent_normalized == mount_root
        };

        if should_show_parent {
            body.push_str("<tr class=\"parent-row\"><td colspan=\"3\">");
            body.push_str(theme::FOLDER_ICON);
            body.push_str("<a href=\"");
            body.push_str(&theme::html_escape(parent));
            if parent != "/" {
                body.push('/');
            }
            body.push_str("\">..</a></td></tr>\n");
        }
    }

    // Entry rows
    let normalized = if uri_path.ends_with('/') {
        uri_path.to_string()
    } else {
        format!("{uri_path}/")
    };

    if entries.is_empty() {
        body.push_str(
            "<tr><td colspan=\"3\" class=\"empty-dir\">\
             This directory is empty</td></tr>\n",
        );
    }

    for entry in entries {
        let name = theme::html_escape(&entry.name);
        let trailing = if entry.is_dir { "/" } else { "" };
        let icon = if entry.is_dir {
            theme::FOLDER_ICON
        } else {
            theme::FILE_ICON
        };
        let size_str = if entry.is_dir {
            "\u{2014}".to_string()
        } else {
            format_size(entry.size)
        };

        body.push_str("<tr data-name=\"");
        body.push_str(&name);
        body.push_str("\" data-dir=\"");
        body.push_str(if entry.is_dir { "1" } else { "0" });
        body.push_str("\" data-size=\"");
        body.push_str(&entry.size.to_string());
        body.push_str("\" data-ts=\"");
        body.push_str(&entry.modified.to_string());
        body.push_str("\"><td>");
        body.push_str(icon);
        body.push_str("<a href=\"");
        body.push_str(&theme::html_escape(&normalized));
        body.push_str(&theme::encode_path_segment(&entry.name));
        body.push_str(trailing);
        body.push_str("\">");
        body.push_str(&name);
        body.push_str(trailing);
        body.push_str("</a></td><td class=\"size\">");
        body.push_str(&size_str);
        body.push_str("</td><td class=\"modified\" data-ts=\"");
        body.push_str(&entry.modified.to_string());
        body.push_str("\"></td></tr>\n");
    }

    body.push_str("</tbody>\n</table>\n</div>");

    theme::render_page(
        &format!("Index of {uri_path}"),
        &body,
        FILEBROWSER_CSS,
        FILEBROWSER_JS,
    )
}

// ── File Browser Styles ─────────────────────────────────────

const FILEBROWSER_CSS: &str = "\
.page-title{\
    font-size:1.1em;color:var(--text-light);margin-bottom:16px;\
    font-weight:400;display:flex;align-items:center;gap:8px;\
}\
.page-title code{\
    font-size:1em;font-weight:600;color:var(--text);\
    background:transparent;padding:0;\
}\
.breadcrumb{\
    display:flex;flex-wrap:wrap;align-items:center;gap:4px;\
    padding:12px 18px;margin-bottom:24px;\
    background:var(--card-bg);border-radius:10px;\
    border:1px solid var(--border);font-size:.9em;\
    box-shadow:0 2px 8px rgba(0,0,0,.03);\
}\
.breadcrumb a{\
    color:var(--fox-orange);padding:4px 8px;border-radius:6px;\
    transition:all .2s ease;\
}\
.breadcrumb a:hover{\
    background:rgba(232,133,58,.12);text-decoration:none;\
    transform:translateY(-1px);\
}\
.bc-home{vertical-align:middle;color:var(--fox-orange)}\
.breadcrumb .sep{color:var(--border-hover);margin:0 4px;font-size:.85em}\
.file-card{\
    background:var(--card-bg);border-radius:12px;\
    border:1px solid var(--border);overflow:hidden;\
    box-shadow:0 4px 16px rgba(0,0,0,.04);\
    animation:fadeIn .5s ease-out;\
}\
.file-card table{width:100%;border-collapse:collapse}\
.file-card th{\
    text-align:left;padding:14px 18px;\
    background:linear-gradient(180deg,#FEFAF6 0%,#FDF6EE 100%);\
    color:var(--text-light);\
    font-size:.75em;font-weight:600;\
    text-transform:uppercase;letter-spacing:.08em;\
    cursor:pointer;user-select:none;\
    border-bottom:2px solid var(--border);\
    transition:all .2s ease;\
}\
.file-card th:hover{color:var(--fox-orange);background:#FFF5ED}\
.file-card td{\
    padding:12px 18px;border-bottom:1px solid #FAF4EE;\
    vertical-align:middle;transition:background .2s ease;\
}\
.file-card tr:last-child td{border-bottom:none}\
.file-card tbody tr:hover td{background:#FFF8F2;cursor:pointer}\
.file-card a{color:var(--text);font-weight:500;transition:all .2s ease}\
.file-card a:hover{color:var(--fox-orange);text-decoration:none}\
.parent-row td{padding:10px 18px;background:rgba(232,133,58,.03)}\
.parent-row:hover td{background:rgba(232,133,58,.08)!important}\
.ei{vertical-align:middle;margin-right:10px;transition:transform .2s}\
.file-card tr:hover .ei{transform:scale(1.1)}\
.size,.modified{\
    color:var(--text-light);\
    font-family:'SF Mono',Monaco,'Cascadia Code',Menlo,Consolas,monospace;\
    font-size:.82em;white-space:nowrap;\
}\
.si{font-size:.75em;margin-left:6px;opacity:.3;transition:all .2s}\
.si.active{opacity:1;color:var(--fox-orange);font-weight:700}\
.empty-dir{padding:48px 18px;text-align:center;color:var(--text-light);font-style:italic}\
.col-size{width:110px}\
.col-mod{width:200px}\
@keyframes fadeIn{from{opacity:0}to{opacity:1}}\
@media(max-width:768px){.col-mod{display:none}.col-size{width:80px}}\
";

// ── File Browser JavaScript ─────────────────────────────────

const FILEBROWSER_JS: &str = "\
document.querySelectorAll('.modified').forEach(function(el){\
    var ts=parseInt(el.dataset.ts);\
    if(ts>0){\
        var d=new Date(ts*1000);\
        el.textContent=d.toLocaleDateString(undefined,\
            {year:'numeric',month:'short',day:'numeric'})\
            +' '+d.toLocaleTimeString([],{hour:'2-digit',minute:'2-digit'});\
    }\
});\
var col=0,asc=true;\
function sort(c){\
    if(col===c)asc=!asc;else{col=c;asc=true;}\
    for(var i=0;i<3;i++){\
        var el=document.getElementById('s'+i);\
        el.className='si'+(i===col?' active':'');\
        el.textContent=i===col?(asc?'\\u25B2':'\\u25BC'):'';\
    }\
    var tbody=document.querySelector('#listing tbody');\
    var nonEntryRows=Array.from(tbody.querySelectorAll('tr:not([data-name])'));\
    var rows=Array.from(tbody.querySelectorAll('tr[data-name]'));\
    rows.sort(function(a,b){\
        var ad=parseInt(a.dataset.dir),bd=parseInt(b.dataset.dir);\
        if(ad!==bd)return bd-ad;\
        var av,bv;\
        if(c===0){\
            av=a.dataset.name.toLowerCase();\
            bv=b.dataset.name.toLowerCase();\
            return asc?av.localeCompare(bv):bv.localeCompare(av);\
        }else if(c===1){\
            av=parseInt(a.dataset.size);bv=parseInt(b.dataset.size);\
        }else{\
            av=parseInt(a.dataset.ts);bv=parseInt(b.dataset.ts);\
        }\
        return asc?av-bv:bv-av;\
    });\
    while(tbody.firstChild)tbody.removeChild(tbody.firstChild);\
    nonEntryRows.forEach(function(r){tbody.appendChild(r);});\
    rows.forEach(function(r){tbody.appendChild(r);});\
}\
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(10240), "10.0 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1073741824), "1.0 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.0 GB");
    }

    #[test]
    fn test_percent_decode() {
        assert_eq!(percent_decode("hello"), "hello");
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("/path%2Fto%2Ffile"), "/path/to/file");
        assert_eq!(percent_decode("%E2%9C%93"), "\u{2713}");
    }

    #[test]
    fn test_percent_decode_invalid() {
        assert_eq!(percent_decode("%GG"), "%GG");
        assert_eq!(percent_decode("%2"), "%2");
        assert_eq!(percent_decode("trailing%"), "trailing%");
    }

    #[test]
    fn test_breadcrumb_root() {
        let bc = build_breadcrumb("/", "/");
        // Should have home icon link and no separators
        assert!(bc.contains("href=\"/\""));
        assert!(!bc.contains("sep"));
    }

    #[test]
    fn test_breadcrumb_nested() {
        let bc = build_breadcrumb("/", "/images/photos/");
        assert!(bc.contains("href=\"/\""));
        assert!(bc.contains("/images/\">images</a>"));
        assert!(bc.contains("/images/photos/\">photos</a>"));
        assert_eq!(bc.matches("class=\"sep\"").count(), 2);
    }

    #[test]
    fn test_breadcrumb_percent_encoded_segments() {
        let bc = build_breadcrumb("/", "/my%20dir/child%2Fslash/");
        // Display should be decoded
        assert!(bc.contains(">my dir</a>"));
        assert!(bc.contains(">child/slash</a>"));
        // Hrefs should be encoded once (no %25 double-encoding)
        assert!(bc.contains("href=\"/my%20dir/\">"));
        assert!(bc.contains("href=\"/my%20dir/child%2Fslash/\">"));
    }

    #[test]
    fn test_breadcrumb_non_root_mount() {
        let bc = build_breadcrumb("/static", "/static/images/");
        // Home icon should link to /static/ not /
        assert!(bc.contains("href=\"/static/\""));
        assert!(bc.contains("/images/\">images</a>"));
    }

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

    #[test]
    fn test_resolve_path_basic() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("subdir");
        fs::create_dir(&sub).unwrap();

        let resolved = resolve_path(tmp.path(), "/subdir");
        assert_eq!(resolved.unwrap(), sub.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_path_root() {
        let tmp = tempfile::tempdir().unwrap();

        let resolved = resolve_path(tmp.path(), "/");
        assert_eq!(resolved.unwrap(), tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_path_traversal_blocked() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("subdir");
        fs::create_dir(&sub).unwrap();

        let resolved = resolve_path(&sub, "/../../../etc/passwd");
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_path_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();

        let resolved = resolve_path(tmp.path(), "/does-not-exist");
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_path_percent_encoded() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("my dir");
        fs::create_dir(&sub).unwrap();

        let resolved = resolve_path(tmp.path(), "/my%20dir");
        assert_eq!(resolved.unwrap(), sub.canonicalize().unwrap());
    }

    #[test]
    fn test_read_directory_sorts_dirs_first() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("b_file.txt"), "hello").unwrap();
        fs::create_dir(tmp.path().join("a_dir")).unwrap();
        fs::write(tmp.path().join("a_file.txt"), "world").unwrap();

        let entries = read_directory(tmp.path()).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "a_dir");
        assert_eq!(entries[1].name, "a_file.txt");
        assert_eq!(entries[2].name, "b_file.txt");
    }

    #[test]
    fn test_read_directory_file_sizes() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("small.txt"), "hi").unwrap();
        fs::create_dir(tmp.path().join("dir")).unwrap();

        let entries = read_directory(tmp.path()).unwrap();
        let dir = entries.iter().find(|e| e.name == "dir").unwrap();
        let file = entries.iter().find(|e| e.name == "small.txt").unwrap();
        assert_eq!(dir.size, 0);
        assert_eq!(file.size, 2);
    }

    #[test]
    fn test_render_directory_listing_contains_entries() {
        let entries = vec![
            DirEntry {
                name: "docs".to_string(),
                is_dir: true,
                size: 0,
                modified: 1700000000,
            },
            DirEntry {
                name: "readme.md".to_string(),
                is_dir: false,
                size: 4096,
                modified: 1700000000,
            },
        ];

        let html = render_directory_listing("/", "/project/", &entries);

        // Themed page structure
        assert!(html.contains("roxy-header"));
        assert!(html.contains("roxy-footer"));
        // Content
        assert!(html.contains("Index of"));
        assert!(html.contains("/project/"));
        assert!(html.contains("docs/</a>"));
        assert!(html.contains("readme.md</a>"));
        assert!(html.contains("4.0 KB"));
    }

    #[test]
    fn test_render_directory_listing_parent_link() {
        let entries = vec![];
        let html = render_directory_listing("/", "/images/photos/", &entries);
        assert!(html.contains(">..</a>"));
        assert!(html.contains("/images/\""));
    }

    #[test]
    fn test_render_directory_listing_no_parent_at_root() {
        let entries = vec![];
        let html = render_directory_listing("/", "/", &entries);
        assert!(!html.contains(".."));
    }

    #[test]
    fn test_render_directory_listing_no_parent_at_mount_root() {
        let entries = vec![];
        let html = render_directory_listing("/static", "/static/", &entries);
        assert!(!html.contains(".."));
    }

    #[test]
    fn test_render_directory_listing_empty_state() {
        let entries = vec![];
        let html = render_directory_listing("/", "/", &entries);
        assert!(html.contains("empty"));
    }

    #[test]
    fn test_render_directory_listing_uses_svg_icons() {
        let entries = vec![
            DirEntry {
                name: "folder".to_string(),
                is_dir: true,
                size: 0,
                modified: 0,
            },
            DirEntry {
                name: "file.txt".to_string(),
                is_dir: false,
                size: 100,
                modified: 0,
            },
        ];

        let html = render_directory_listing("/", "/", &entries);
        // Should use SVG icons, not emoji
        assert!(html.contains(r##"fill="#E8853A""##)); // folder orange
        assert!(html.contains(r##"fill="#3BB8A2""##)); // file teal
    }

    #[tokio::test]
    async fn test_try_directory_listing_for_directory() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("file.txt"), "content").unwrap();
        fs::create_dir(tmp.path().join("sub")).unwrap();

        let resolved = resolve_path(tmp.path(), "/").unwrap();
        let response = try_directory_listing("/", "/", resolved).await;
        assert!(response.is_some());
        assert_eq!(response.unwrap().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_try_directory_listing_none_for_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("file.txt"), "content").unwrap();

        let resolved = resolve_path(tmp.path(), "/file.txt").unwrap();
        let response = try_directory_listing("/", "/file.txt", resolved).await;
        assert!(response.is_none());
    }
}
