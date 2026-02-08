use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PathPrefix(String);

#[derive(Debug, Error)]
pub enum PathPrefixError {
    #[error("Path prefix must start with '/'")]
    MustStartWithSlash,

    #[error("Path prefix cannot have trailing slash (except for '/')")]
    TrailingSlash,

    #[error("Path prefix contains invalid characters")]
    InvalidCharacters,
}

impl PathPrefix {
    pub fn new(path: impl Into<String>) -> Result<Self, PathPrefixError> {
        let path = path.into();

        // Must start with "/"
        if !path.starts_with('/') {
            return Err(PathPrefixError::MustStartWithSlash);
        }

        // No trailing slash (except for "/" itself)
        if path.len() > 1 && path.ends_with('/') {
            return Err(PathPrefixError::TrailingSlash);
        }

        // Validate URL path characters (alphanumeric, -, _, ., /, ~)
        let valid_chars = |c: char| c.is_ascii_alphanumeric() || "-_.~/".contains(c);
        if !path.chars().all(valid_chars) {
            return Err(PathPrefixError::InvalidCharacters);
        }

        Ok(Self(path))
    }

    /// Check if this prefix matches a request path.
    /// Returns true if the request path starts with this prefix,
    /// followed by either end of string or '/'.
    pub fn matches(&self, request_path: &str) -> bool {
        if self.0 == "/" {
            // Root matches everything
            true
        } else {
            request_path.starts_with(&self.0)
                && (request_path.len() == self.0.len()
                    || request_path.as_bytes().get(self.0.len()) == Some(&b'/'))
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for PathPrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_prefixes() {
        assert!(PathPrefix::new("/").is_ok());
        assert!(PathPrefix::new("/api").is_ok());
        assert!(PathPrefix::new("/api/v1").is_ok());
        assert!(PathPrefix::new("/my-app").is_ok());
        assert!(PathPrefix::new("/my_app").is_ok());
    }

    #[test]
    fn test_invalid_prefixes() {
        assert!(PathPrefix::new("api").is_err()); // Must start with /
        assert!(PathPrefix::new("/api/").is_err()); // No trailing slash
        assert!(PathPrefix::new("/api?").is_err()); // Invalid char
    }

    #[test]
    fn test_root_matches_everything() {
        let root = PathPrefix::new("/").unwrap();
        assert!(root.matches("/"));
        assert!(root.matches("/api"));
        assert!(root.matches("/api/v1"));
        assert!(root.matches("/anything/at/all"));
    }

    #[test]
    fn test_prefix_matching() {
        let api = PathPrefix::new("/api").unwrap();

        // Should match
        assert!(api.matches("/api"));
        assert!(api.matches("/api/"));
        assert!(api.matches("/api/users"));
        assert!(api.matches("/api/users/123"));

        // Should NOT match
        assert!(!api.matches("/"));
        assert!(!api.matches("/apiv2")); // No boundary
        assert!(!api.matches("/application"));
        assert!(!api.matches("/other"));
    }

    #[test]
    fn test_nested_prefix_matching() {
        let api_v1 = PathPrefix::new("/api/v1").unwrap();

        assert!(api_v1.matches("/api/v1"));
        assert!(api_v1.matches("/api/v1/users"));
        assert!(!api_v1.matches("/api"));
        assert!(!api_v1.matches("/api/v2"));
    }
}
