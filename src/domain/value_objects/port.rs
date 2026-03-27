use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port(u16);

#[derive(Debug, thiserror::Error)]
pub enum PortError {
    #[error("Port must be between 1 and 65535, got: {0}")]
    OutOfRange(u16),

    #[error("Privileged ports (1-1023) are not allowed for target services, got: {0}")]
    Privileged(u16),
}

impl Port {
    pub fn new(port: u16) -> Result<Self, PortError> {
        if port == 0 {
            return Err(PortError::OutOfRange(port));
        }
        if port < 1024 {
            return Err(PortError::Privileged(port));
        }
        Ok(Self(port))
    }

    /// Create a port allowing the full 1..=65535 range.
    ///
    /// Use for targets where Roxy connects as a client (e.g., container
    /// ports mapped to the host) rather than user-configured services.
    pub fn any(port: u16) -> Result<Self, PortError> {
        if port == 0 {
            return Err(PortError::OutOfRange(port));
        }
        Ok(Self(port))
    }

    pub fn value(&self) -> u16 {
        self.0
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for Port {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Port {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let port = u16::deserialize(deserializer)?;
        Port::new(port).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_ports() {
        assert!(Port::new(3000).is_ok());
        assert!(Port::new(8080).is_ok());
        assert!(Port::new(65535).is_ok());
    }

    #[test]
    fn test_invalid_ports() {
        assert!(Port::new(0).is_err());
        assert!(Port::new(80).is_err()); // Privileged
        assert!(Port::new(443).is_err()); // Privileged
    }

    #[test]
    fn test_any_port_allows_privileged() {
        assert!(Port::any(80).is_ok());
        assert!(Port::any(443).is_ok());
        assert_eq!(Port::any(80).unwrap().value(), 80);
    }

    #[test]
    fn test_any_port_rejects_zero() {
        assert!(Port::any(0).is_err());
    }

    #[test]
    fn test_any_port_allows_high_ports() {
        assert!(Port::any(3000).is_ok());
        assert!(Port::any(65535).is_ok());
    }
}
