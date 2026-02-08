use super::path_prefix::{PathPrefix, PathPrefixError};
use super::proxy_target::{ProxyTarget, ProxyTargetError};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Route {
    pub path: PathPrefix,
    pub target: RouteTarget,
}

#[derive(Debug, Clone)]
pub enum RouteTarget {
    Proxy(ProxyTarget),
    StaticFiles(PathBuf),
}

#[derive(Debug, Error)]
pub enum RouteTargetError {
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Invalid proxy target: {0}")]
    InvalidProxyTarget(#[from] ProxyTargetError),
}

#[derive(Debug, Error)]
#[allow(clippy::enum_variant_names)]
pub enum RouteError {
    #[error("Invalid path prefix: {0}")]
    InvalidPathPrefix(#[from] PathPrefixError),

    #[error("Invalid target: {0}")]
    InvalidTarget(#[from] RouteTargetError),

    #[error("Invalid route format: expected 'PATH=TARGET', got '{0}'")]
    InvalidFormat(String),
}

impl RouteTarget {
    /// Parse target string: absolute path (starting with /) = static files, otherwise proxy
    /// Note: To distinguish from PathPrefix, static file paths must exist on disk
    pub fn parse(s: &str) -> Result<Self, RouteTargetError> {
        // If it starts with / and looks like a filesystem path, try static files
        if s.starts_with('/') {
            let path = PathBuf::from(s);
            if path.exists() {
                if !path.is_dir() {
                    return Err(RouteTargetError::NotADirectory(path));
                }
                return Ok(Self::StaticFiles(path.canonicalize().unwrap_or(path)));
            }
            // Path doesn't exist - could be a typo, report it
            return Err(RouteTargetError::PathNotFound(path));
        }

        // Otherwise it's a proxy target
        Ok(Self::Proxy(ProxyTarget::parse(s)?))
    }
}

impl fmt::Display for RouteTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteTarget::Proxy(p) => write!(f, "{}", p),
            RouteTarget::StaticFiles(p) => write!(f, "{}", p.display()),
        }
    }
}

impl Serialize for RouteTarget {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for RouteTarget {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // For deserialization, we need to handle paths that may not exist yet
        // (e.g., loading old config). So we're more lenient here.
        if s.starts_with('/') {
            // Assume it's a static files path
            Ok(Self::StaticFiles(PathBuf::from(&s)))
        } else {
            ProxyTarget::parse(&s)
                .map(Self::Proxy)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl Route {
    pub fn new(path: PathPrefix, target: RouteTarget) -> Self {
        Self { path, target }
    }

    /// Parse from CLI format: "PATH=TARGET" e.g., "/api=3001" or "/=3000"
    pub fn parse(s: &str) -> Result<Self, RouteError> {
        let (path_str, target_str) = s
            .split_once('=')
            .ok_or_else(|| RouteError::InvalidFormat(s.to_string()))?;

        let path = PathPrefix::new(path_str)?;
        let target = RouteTarget::parse(target_str)?;

        Ok(Self { path, target })
    }
}

impl Serialize for Route {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Route", 2)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("target", &self.target)?;
        s.end()
    }
}

impl<'de> Deserialize<'de> for Route {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RouteHelper {
            path: PathPrefix,
            target: RouteTarget,
        }

        let helper = RouteHelper::deserialize(deserializer)?;
        Ok(Self {
            path: helper.path,
            target: helper.target,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_proxy_route() {
        let route = Route::parse("/api=3001").unwrap();
        assert_eq!(route.path.to_string(), "/api");
        assert!(matches!(route.target, RouteTarget::Proxy(_)));
    }

    #[test]
    fn test_parse_root_route() {
        let route = Route::parse("/=3000").unwrap();
        assert_eq!(route.path.to_string(), "/");
    }

    #[test]
    fn test_parse_with_host() {
        let route = Route::parse("/api=192.168.1.50:3001").unwrap();
        let RouteTarget::Proxy(proxy) = &route.target else {
            panic!("expected proxy target");
        };
        assert_eq!(proxy.host(), "192.168.1.50");
        assert_eq!(proxy.port().value(), 3001);
    }

    #[test]
    fn test_invalid_format() {
        assert!(Route::parse("no-equals-sign").is_err());
        assert!(Route::parse("").is_err());
    }

    #[test]
    fn test_route_target_display() {
        let proxy = RouteTarget::Proxy(ProxyTarget::parse("3000").unwrap());
        assert_eq!(proxy.to_string(), "127.0.0.1:3000");

        let static_files = RouteTarget::StaticFiles(PathBuf::from("/var/www"));
        assert_eq!(static_files.to_string(), "/var/www");
    }
}
