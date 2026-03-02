/// Port for system-level directory and file operations.
pub trait SystemSetup {
    /// Create required data, certs, and log directories.
    fn create_directories(&self) -> anyhow::Result<()>;

    /// Remove the data directory. Returns true if it existed.
    fn remove_data_directory(&self) -> anyhow::Result<bool>;

    /// Remove the PID file. Returns true if it existed.
    fn remove_pid_file(&self) -> bool;

    /// Remove the log directory. Returns true if it existed.
    fn remove_log_directory(&self) -> bool;
}
