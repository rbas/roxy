use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::application::ports::RegistrationProvider;
use crate::domain::DomainRegistration;
use crate::infrastructure::config::ConfigStore;

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Supplies domain registrations from the config file on disk.
pub struct ConfigFileProvider {
    config_store: ConfigStore,
    config_path: PathBuf,
    poll_interval: Duration,
}

impl ConfigFileProvider {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            config_store: ConfigStore::new(config_path.clone()),
            config_path,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }

    /// Set the poll interval for file change detection.
    /// Useful for tests that need fast feedback.
    #[cfg(test)]
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Poll the config file for mtime changes and send a nudge
    /// through the channel when the file is modified.
    pub async fn watch(&self, tx: mpsc::Sender<()>, cancel: CancellationToken) {
        let mut last_mtime = file_mtime(&self.config_path);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(self.poll_interval) => {}
            }

            let current_mtime = file_mtime(&self.config_path);
            if current_mtime == last_mtime {
                continue;
            }
            last_mtime = current_mtime;

            // Verify the file is parseable before nudging
            match self.load() {
                Ok(registrations) => {
                    let count = registrations.len();
                    if tx.send(()).await.is_err() {
                        break; // receiver dropped
                    }
                    info!(count, "Config file changed, nudging reload");
                }
                Err(e) => {
                    warn!(error = %e, "Config file changed but failed to load, keeping old state");
                }
            }
        }
    }
}

impl RegistrationProvider for ConfigFileProvider {
    fn name(&self) -> &str {
        "config-file"
    }

    fn load(&self) -> anyhow::Result<Vec<DomainRegistration>> {
        let config = self.config_store.load()?;
        Ok(config.registrations())
    }
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_file_provider_name() {
        let provider = ConfigFileProvider::new(PathBuf::from("/tmp/nonexistent.toml"));
        assert_eq!(provider.name(), "config-file");
    }

    #[test]
    fn config_file_provider_load_missing_file() {
        let provider = ConfigFileProvider::new(PathBuf::from("/tmp/nonexistent-roxy-test.toml"));
        let result = provider.load();
        // Missing file returns empty default config with no domains
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn with_poll_interval_overrides_default() {
        let provider = ConfigFileProvider::new(PathBuf::from("/tmp/test.toml"))
            .with_poll_interval(Duration::from_millis(10));
        assert_eq!(provider.poll_interval, Duration::from_millis(10));
    }

    #[tokio::test]
    async fn watch_detects_file_change() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "").unwrap();

        let provider = ConfigFileProvider::new(config_path.clone())
            .with_poll_interval(Duration::from_millis(10));
        let (tx, mut rx) = mpsc::channel::<()>(4);
        let cancel = CancellationToken::new();

        let cancel_watch = cancel.clone();
        let handle = tokio::spawn(async move {
            provider.watch(tx, cancel_watch).await;
        });

        // Ensure at least one poll cycle sees the original mtime
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Modify the file — add a domain registration
        std::fs::write(
            &config_path,
            r#"
[domains."app.roxy"]
pattern = "app.roxy"
routes = [{ path = "/", target = "3000" }]
"#,
        )
        .unwrap();

        // Should receive a nudge within a few poll cycles
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for reload")
            .expect("channel closed");

        cancel.cancel();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn watch_ignores_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "").unwrap();

        let provider =
            ConfigFileProvider::new(config_path).with_poll_interval(Duration::from_millis(10));
        let (tx, mut rx) = mpsc::channel::<()>(4);
        let cancel = CancellationToken::new();

        let cancel_watch = cancel.clone();
        tokio::spawn(async move {
            provider.watch(tx, cancel_watch).await;
        });

        // Wait several poll cycles without modifying the file
        tokio::time::sleep(Duration::from_millis(60)).await;

        // Channel should be empty — no spurious reloads
        assert!(rx.try_recv().is_err());

        cancel.cancel();
    }

    #[tokio::test]
    async fn watch_stops_on_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "").unwrap();

        let provider =
            ConfigFileProvider::new(config_path).with_poll_interval(Duration::from_millis(10));
        let (tx, _rx) = mpsc::channel::<()>(4);
        let cancel = CancellationToken::new();

        let cancel_watch = cancel.clone();
        let handle = tokio::spawn(async move {
            provider.watch(tx, cancel_watch).await;
        });

        cancel.cancel();

        // watch() should exit promptly
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("watch did not stop after cancel")
            .unwrap();
    }

    #[tokio::test]
    async fn watch_survives_invalid_config() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        std::fs::write(&config_path, "").unwrap();

        let provider = ConfigFileProvider::new(config_path.clone())
            .with_poll_interval(Duration::from_millis(10));
        let (tx, mut rx) = mpsc::channel::<()>(4);
        let cancel = CancellationToken::new();

        let cancel_watch = cancel.clone();
        tokio::spawn(async move {
            provider.watch(tx, cancel_watch).await;
        });

        // Wait for initial poll
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Write invalid TOML — watcher should log warning but keep running
        std::fs::write(&config_path, "not [valid toml").unwrap();
        tokio::time::sleep(Duration::from_millis(60)).await;

        // No message should be sent (parse failure keeps old state)
        assert!(rx.try_recv().is_err());

        // Now write valid config — watcher should recover and send a nudge
        std::fs::write(
            &config_path,
            r#"
[domains."fixed.roxy"]
pattern = "fixed.roxy"
routes = [{ path = "/", target = "4000" }]
"#,
        )
        .unwrap();

        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timed out waiting for recovery reload")
            .expect("channel closed");

        cancel.cancel();
    }
}
