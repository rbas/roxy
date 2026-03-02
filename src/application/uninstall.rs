use std::time::Duration;

use anyhow::Result;

use super::StepOutcome;
use super::ports::{CertificateManager, DaemonControl, DnsManager, DomainRepository, SystemSetup};

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
    domains: &'a dyn DomainRepository,
    certs: &'a dyn CertificateManager,
    daemon: &'a dyn DaemonControl,
    dns: &'a dyn DnsManager,
    system: &'a dyn SystemSetup,
    data_dir_display: String,
}

impl<'a> Uninstall<'a> {
    pub fn new(
        domains: &'a dyn DomainRepository,
        certs: &'a dyn CertificateManager,
        daemon: &'a dyn DaemonControl,
        dns: &'a dyn DnsManager,
        system: &'a dyn SystemSetup,
        data_dir_display: String,
    ) -> Self {
        Self {
            domains,
            certs,
            daemon,
            dns,
            system,
            data_dir_display,
        }
    }

    /// Build a preview so the CLI can show a confirmation prompt.
    pub fn preview(&self) -> Result<UninstallPreview> {
        let domain_count = self.domains.list().unwrap_or_default().len();
        Ok(UninstallPreview {
            domain_count,
            data_dir: self.data_dir_display.clone(),
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
        if self.daemon.get_running_pid()?.is_some() {
            self.daemon.stop_gracefully(Duration::from_millis(500))?;
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
        let domains = self.domains.list().unwrap_or_default();

        for registration in &domains {
            let label = format!("Remove cert: {}", registration.display_pattern());
            let outcome = match self.certs.remove(registration.pattern()) {
                Ok(_) => StepOutcome::Success("Removed.".into()),
                Err(e) => StepOutcome::Warning(format!("Failed: {}", e)),
            };
            steps.push((label, outcome));
        }

        let ca_outcome = match self.certs.remove_ca() {
            Ok(_) => StepOutcome::Success("Root CA removed.".into()),
            Err(e) => StepOutcome::Warning(format!("Failed to remove Root CA: {}", e)),
        };
        steps.push(("Remove Root CA".into(), ca_outcome));
    }

    fn remove_dns(&self, steps: &mut Vec<(String, StepOutcome)>) -> Result<()> {
        if self.dns.is_configured() {
            self.dns.cleanup()?;
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
        if self.system.remove_data_directory()? {
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
        if self.system.remove_pid_file() {
            steps.push((
                "Remove PID file".into(),
                StepOutcome::Success("PID file removed.".into()),
            ));
        }

        if self.system.remove_log_directory() {
            steps.push((
                "Remove log directory".into(),
                StepOutcome::Success("Log directory removed.".into()),
            ));
        }
    }
}
