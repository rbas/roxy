// Docker items are used from daemon/lifecycle.rs (bin crate) but the
// lib crate doesn't see the usage, producing dead_code warnings.
#[allow(dead_code)]
pub(crate) mod discovery;
#[allow(dead_code)]
pub(crate) mod network;
pub mod provider;
#[allow(dead_code)]
pub(crate) mod watcher;

pub use provider::DockerProvider;
