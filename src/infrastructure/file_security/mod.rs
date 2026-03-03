use std::path::Path;

/// Platform-agnostic file security operations.
///
/// Implementations MUST succeed or return an error — no silent no-ops.
pub trait FileSecurity {
    /// Restrict a file to owner-only read/write (0600 on Unix).
    fn restrict_key_permissions(&self, path: &Path) -> std::io::Result<()>;
}

#[cfg(unix)]
mod unix;

/// Get the file security provider for the current platform.
#[cfg(unix)]
pub fn get_file_security() -> impl FileSecurity {
    unix::UnixFileSecurity
}

/// Get the file security provider for the current platform.
#[cfg(not(unix))]
pub fn get_file_security() -> impl FileSecurity {
    UnsupportedFileSecurity
}

/// Convenience function: restrict a file to owner-only read/write.
///
/// Delegates to the platform file security provider.
pub fn restrict_key_permissions(path: &Path) -> std::io::Result<()> {
    get_file_security().restrict_key_permissions(path)
}

/// Fallback for unsupported platforms — always returns an error.
#[cfg(not(unix))]
struct UnsupportedFileSecurity;

#[cfg(not(unix))]
impl FileSecurity for UnsupportedFileSecurity {
    fn restrict_key_permissions(&self, _path: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!(
                "File permission restriction not implemented for {}. \
                 Key files may be world-readable.",
                std::env::consts::OS
            ),
        ))
    }
}
