use std::path::{Path, PathBuf};

/// Resolve a URI path to a filesystem path within the root directory.
///
/// Returns `None` if the path doesn't exist or escapes the root
/// (path traversal protection).
pub(super) fn resolve_path(root: &Path, uri_path: &str) -> Option<PathBuf> {
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
pub(super) fn percent_decode(s: &str) -> String {
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

/// Format bytes as a human-readable size string.
pub(super) fn format_size(bytes: u64) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
}
