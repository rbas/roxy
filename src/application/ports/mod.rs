pub mod certificate_manager;
pub mod config_loader;
pub mod daemon_connection;
pub mod daemon_control;
pub mod dns_manager;
pub mod domain_repository;
pub mod network_info;
pub mod registration_provider;
pub mod system_setup;

pub use certificate_manager::{CertificateError, CertificateManager};
pub use config_loader::{ConfigLoadError, ConfigLoader};
#[allow(unused_imports)] // Used by testkit; CLI adapter comes later.
pub use daemon_connection::{DaemonConnection, DaemonConnectionError, DaemonStatus};
pub use daemon_control::DaemonControl;
pub use dns_manager::{DnsConfigError, DnsManager};
pub use domain_repository::{DomainRepository, RepositoryError};
pub use network_info::NetworkInfo;
pub use registration_provider::RegistrationProvider;
pub use system_setup::SystemSetup;
