use std::fs;

use crate::application::ports::SystemSetup;
use crate::infrastructure::paths::RoxyPaths;

/// Infrastructure service for filesystem operations required by Roxy.
pub struct FileSystemSetup<'a> {
    paths: &'a RoxyPaths,
}

impl<'a> FileSystemSetup<'a> {
    pub fn new(paths: &'a RoxyPaths) -> Self {
        Self { paths }
    }
}

impl SystemSetup for FileSystemSetup<'_> {
    fn create_directories(&self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.paths.data_dir).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create data directory {}: {}",
                self.paths.data_dir.display(),
                e
            )
        })?;

        fs::create_dir_all(&self.paths.certs_dir).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create certs directory {}: {}",
                self.paths.certs_dir.display(),
                e
            )
        })?;

        if let Some(log_dir) = self.paths.log_file.parent() {
            fs::create_dir_all(log_dir).map_err(|e| {
                anyhow::anyhow!(
                    "Failed to create log directory {}: {}",
                    log_dir.display(),
                    e
                )
            })?;
        }

        Ok(())
    }

    fn remove_data_directory(&self) -> anyhow::Result<bool> {
        if self.paths.data_dir.exists() {
            fs::remove_dir_all(&self.paths.data_dir)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn remove_pid_file(&self) -> bool {
        fs::remove_file(&self.paths.pid_file).is_ok()
    }

    fn remove_log_directory(&self) -> bool {
        self.paths
            .log_file
            .parent()
            .is_some_and(|log_dir| fs::remove_dir_all(log_dir).is_ok())
    }
}
