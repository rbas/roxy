use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::FileSecurity;

pub struct UnixFileSecurity;

impl FileSecurity for UnixFileSecurity {
    fn restrict_key_permissions(&self, path: &Path) -> std::io::Result<()> {
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(path, perms)
    }
}
