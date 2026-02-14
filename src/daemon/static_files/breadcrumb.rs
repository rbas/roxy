use super::path_utils::percent_decode;
use crate::daemon::theme;

/// Build a breadcrumb navigation bar from the URI path.
///
/// The `route_prefix` is used as the root href to keep navigation within
/// the mounted route (e.g., `/static/` for non-root mounts).
pub(super) fn build_breadcrumb(route_prefix: &str, uri_path: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
