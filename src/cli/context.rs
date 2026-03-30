use std::path::Path;

use crate::infrastructure::certs::CertificateService;
use crate::infrastructure::config::ConfigStore;
use crate::infrastructure::mgmt_client::MgmtSocketClient;
use crate::infrastructure::paths::RoxyPaths;
use crate::infrastructure::pid::PidFile;

/// Composition root: wires infrastructure adapters once,
/// shared by all CLI commands that need them.
pub struct AppContext {
    pub config_store: ConfigStore,
    pub cert_service: CertificateService,
    pub pid_file: PidFile,
    pub mgmt_client: MgmtSocketClient,
}

impl AppContext {
    pub fn new(config_path: &Path, paths: &RoxyPaths) -> Self {
        Self {
            config_store: ConfigStore::new(config_path.to_path_buf()),
            cert_service: CertificateService::new(paths),
            pid_file: PidFile::new(paths.pid_file.clone()),
            mgmt_client: MgmtSocketClient::new(&paths.socket_path),
        }
    }
}
