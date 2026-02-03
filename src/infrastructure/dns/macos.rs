use super::{DnsError, DnsService};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

const RESOLVER_DIR: &str = "/etc/resolver";
const RESOLVER_FILE: &str = "/etc/resolver/roxy";
const RESOLVER_CONTENT: &str = "nameserver 127.0.0.1\n";

pub struct MacOsDnsService;

impl MacOsDnsService {
    pub fn new() -> Self {
        Self
    }

    fn ensure_resolver_dir(&self) -> Result<(), DnsError> {
        let path = Path::new(RESOLVER_DIR);
        if !path.exists() {
            fs::create_dir_all(path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DnsError::PermissionDenied
                } else {
                    DnsError::WriteError {
                        path: path.to_path_buf(),
                        source: e,
                    }
                }
            })?;
        }
        Ok(())
    }
}

impl Default for MacOsDnsService {
    fn default() -> Self {
        Self::new()
    }
}

impl DnsService for MacOsDnsService {
    fn setup(&self) -> Result<(), DnsError> {
        self.ensure_resolver_dir()?;

        fs::write(RESOLVER_FILE, RESOLVER_CONTENT).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                DnsError::PermissionDenied
            } else {
                DnsError::WriteError {
                    path: RESOLVER_FILE.into(),
                    source: e,
                }
            }
        })?;

        Ok(())
    }

    fn cleanup(&self) -> Result<(), DnsError> {
        let path = Path::new(RESOLVER_FILE);
        if path.exists() {
            fs::remove_file(path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    DnsError::PermissionDenied
                } else {
                    DnsError::RemoveError {
                        path: path.to_path_buf(),
                        source: e,
                    }
                }
            })?;
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), DnsError> {
        // First, verify the resolver file exists and has correct content
        let path = Path::new(RESOLVER_FILE);
        if !path.exists() {
            return Err(DnsError::ValidationFailed(
                "Resolver file does not exist at /etc/resolver/roxy".into(),
            ));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| DnsError::ValidationFailed(format!("Failed to read resolver file: {}", e)))?;

        if !content.contains("nameserver") || !content.contains("127.0.0.1") {
            return Err(DnsError::ValidationFailed(
                "Resolver file has incorrect content".into(),
            ));
        }

        // Use scutil --dns to verify macOS has registered the resolver
        // Retry a few times as macOS may take a moment to pick up the change
        const MAX_RETRIES: u32 = 5;
        const RETRY_DELAY_MS: u64 = 500;

        for attempt in 1..=MAX_RETRIES {
            let output = Command::new("scutil")
                .args(["--dns"])
                .output()
                .map_err(|e| DnsError::ValidationFailed(format!("Failed to run scutil: {}", e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check if our resolver is listed (flexible spacing check)
            let has_roxy_resolver = stdout.lines().any(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("domain") && trimmed.contains("roxy")
            });

            if has_roxy_resolver {
                return Ok(());
            }

            if attempt < MAX_RETRIES {
                thread::sleep(Duration::from_millis(RETRY_DELAY_MS));
            }
        }

        Err(DnsError::ValidationFailed(
            "DNS resolver for .roxy not found in scutil output after multiple attempts. \
             Try running 'sudo killall -HUP mDNSResponder' to refresh DNS.".into(),
        ))
    }

    fn is_configured(&self) -> bool {
        Path::new(RESOLVER_FILE).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_content_format() {
        assert!(RESOLVER_CONTENT.ends_with('\n'));
        assert!(RESOLVER_CONTENT.contains("nameserver"));
        assert!(RESOLVER_CONTENT.contains("127.0.0.1"));
    }
}
