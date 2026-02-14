use std::fs;
use std::time::Duration;

use anyhow::Result;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::dns::get_dns_service;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

use super::StepOutcome;

/// What will be removed — shown to the user for confirmation.
pub struct UninstallPreview {
    pub domain_count: usize,
    pub data_dir: String,
}

/// Result of the uninstall operation.
pub struct UninstallResult {
    pub steps: Vec<(String, StepOutcome)>,
}

/// Use case: remove all Roxy configuration from the system.
pub struct Uninstall<'a> {
    config_store: &'a ConfigStore,
    cert_service: &'a CertificateService,
    paths: &'a RoxyPaths,
}

impl<'a> Uninstall<'a> {
    pub fn new(
        config_store: &'a ConfigStore,
        cert_service: &'a CertificateService,
        paths: &'a RoxyPaths,
    ) -> Self {
        Self {
            config_store,
            cert_service,
            paths,
        }
    }

    /// Build a preview so the CLI can show a confirmation prompt.
    pub fn preview(&self) -> Result<UninstallPreview> {
        let domain_count = self.config_store.list_domains().unwrap_or_default().len();
        Ok(UninstallPreview {
            domain_count,
            data_dir: self.paths.data_dir.display().to_string(),
        })
    }

    /// Perform the full uninstall: stop daemon, remove certs, DNS,
    /// data directory, PID file, and logs.
    pub fn execute(&self) -> Result<UninstallResult> {
        let mut steps: Vec<(String, StepOutcome)> = Vec::new();

        self.stop_daemon(&mut steps)?;
        self.remove_certificates(&mut steps);
        self.remove_dns(&mut steps)?;
        self.remove_data(&mut steps)?;
        self.cleanup_files(&mut steps);

        Ok(UninstallResult { steps })
    }

    fn stop_daemon(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        let pid_file = PidFile::new(self.paths.pid_file.clone());
        if pid_file.get_running_pid()?.is_some() {
            pid_file.stop_gracefully(Duration::from_millis(500))?;
            steps.push((
                "Stop daemon".into(),
                StepOutcome::Success("Daemon stopped.".into()),
            ));
        } else {
            steps.push((
                "Stop daemon".into(),
                StepOutcome::Skipped("Daemon not running.".into()),
            ));
        }
        Ok(())
    }

    fn remove_certificates(&self, steps: &mut Vec<(String, StepOutcome)>) {
        let domains = self.config_store.list_domains().unwrap_or_default();

        for registration in &domains {
            let label = format!("Remove cert: {}", registration.display_pattern());
            let outcome = match self.cert_service.remove(registration.pattern()) {
                Ok(_) => StepOutcome::Success("Removed.".into()),
                Err(e) => StepOutcome::Warning(format!("Failed: {}", e)),
            };
            steps.push((label, outcome));
        }

        let ca_outcome = match self.cert_service.remove_ca() {
            Ok(_) => StepOutcome::Success("Root CA removed.".into()),
            Err(e) => StepOutcome::Warning(format!("Failed to remove Root CA: {}", e)),
        };
        steps.push(("Remove Root CA".into(), ca_outcome));
    }

    fn remove_dns(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        let dns = get_dns_service()?;
        if dns.is_configured() {
            dns.cleanup()?;
            steps.push((
                "Remove DNS".into(),
                StepOutcome::Success("DNS configuration removed.".into()),
            ));
        } else {
            steps.push((
                "Remove DNS".into(),
                StepOutcome::Skipped("DNS not configured.".into()),
            ));
        }
        Ok(())
    }

    fn remove_data(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        if self.paths.data_dir.exists() {
            fs::remove_dir_all(&self.paths.data_dir)?;
            steps.push((
                "Remove data directory".into(),
                StepOutcome::Success("Directory removed.".into()),
            ));
        } else {
            steps.push((
                "Remove data directory".into(),
                StepOutcome::Skipped("Directory does not exist.".into()),
            ));
        }
        Ok(())
    }

    fn cleanup_files(&self, steps: &mut Vec<(String, StepOutcome)>) {
        if fs::remove_file(&self.paths.pid_file).is_ok() {
            steps.push((
                "Remove PID file".into(),
                StepOutcome::Success("PID file removed.".into()),
            ));
        }

        if let Some(log_dir) = self.paths.log_file.parent()
            && fs::remove_dir_all(log_dir).is_ok()
        {
            steps.push((
                "Remove log directory".into(),
                StepOutcome::Success("Log directory removed.".into()),
            ));
        }
    }
}
