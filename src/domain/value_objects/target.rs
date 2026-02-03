use super::port::{Port, PortError};
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Target {
    #[serde(rename = "path")]
    Path(PathBuf),

    #[serde(rename = "port")]
    Port(Port),
}

#[derive(Debug, thiserror::Error)]
pub enum TargetError {
    #[error("Path does not exist: {0}")]
    PathNotFound(PathBuf),

    #[error("Path is not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Port error: {0}")]
    PortError(#[from] PortError),
}

impl Target {
    pub fn path(path: PathBuf) -> Result<Self, TargetError> {
        if !path.exists() {
            return Err(TargetError::PathNotFound(path));
        }
        if !path.is_dir() {
            return Err(TargetError::NotADirectory(path));
        }
        Ok(Self::Path(path.canonicalize().unwrap_or(path)))
    }

    pub fn port(port: u16) -> Result<Self, TargetError> {
        Ok(Self::Port(Port::new(port)?))
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Path(p) => write!(f, "{}", p.display()),
            Target::Port(p) => write!(f, "localhost:{}", p),
        }
    }
}
