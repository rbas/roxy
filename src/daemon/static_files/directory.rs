use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use axum::http::StatusCode;
use axum::response::Response;
use tokio::task;

use super::breadcrumb::build_breadcrumb;
use super::path_utils::format_size;
use super::styles::{FILEBROWSER_CSS, FILEBROWSER_JS};
use crate::daemon::theme;

pub(super) struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
}

/// Read a directory and collect entries with metadata.
pub(super) fn read_directory(path: &Path) -> Option<Vec<DirEntry>> {
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

/// Try to render a directory listing for `resolved` and show it as `display_path`.
pub(super) async fn try_directory_listing(
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

    render_parent_link(route_prefix, uri_path, &mut body);
    render_entries(uri_path, entries, &mut body);

    body.push_str("</tbody>\n</table>\n</div>");

    theme::render_page(
        &format!("Index of {uri_path}"),
        &body,
        FILEBROWSER_CSS,
        FILEBROWSER_JS,
    )
}

fn render_parent_link(route_prefix: &str, uri_path: &str, body: &mut String) {
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
    if is_at_mount_root {
        return;
    }

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

fn render_entries(uri_path: &str, entries: &[DirEntry], body: &mut String) {
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
}

#[cfg(test)]
mod tests {
    use super::super::path_utils::resolve_path;
    use super::*;

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
