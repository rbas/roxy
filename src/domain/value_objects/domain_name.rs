use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DomainName(String);

#[derive(Debug, thiserror::Error)]
pub enum DomainNameError {
    #[error("Domain must end with '.roxy', got: {0}")]
    InvalidSuffix(String),

    #[error("Domain name too short: {0}")]
    TooShort(String),

    #[error("Domain name contains invalid characters: {0}")]
    InvalidCharacters(String),
}

impl DomainName {
    pub fn new(name: impl Into<String>) -> Result<Self, DomainNameError> {
        let name = name.into().to_lowercase();

        // Must end with .roxy
        if !name.ends_with(".roxy") {
            return Err(DomainNameError::InvalidSuffix(name));
        }

        // Must have at least one character before .roxy
        if name.len() < 6 {
            return Err(DomainNameError::TooShort(name));
        }

        // Extract prefix (part before .roxy)
        let prefix = &name[..name.len() - 5];

        // Validate characters (alphanumeric, hyphens, dots for subdomains)
        if !prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return Err(DomainNameError::InvalidCharacters(name));
        }

        // Cannot start or end with hyphen or dot
        if prefix.starts_with('-')
            || prefix.starts_with('.')
            || prefix.ends_with('-')
            || prefix.ends_with('.')
        {
            return Err(DomainNameError::InvalidCharacters(name));
        }

        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl serde::Serialize for DomainName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for DomainName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DomainName::new(s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_domain_names() {
        assert!(DomainName::new("app.roxy").is_ok());
        assert!(DomainName::new("my-app.roxy").is_ok());
        assert!(DomainName::new("sub.domain.roxy").is_ok());
        assert!(DomainName::new("APP.ROXY").is_ok()); // Should lowercase
    }

    #[test]
    fn test_invalid_domain_names() {
        assert!(DomainName::new("app.local").is_err()); // Wrong suffix
        assert!(DomainName::new(".roxy").is_err()); // Too short
        assert!(DomainName::new("-app.roxy").is_err()); // Starts with hyphen
        assert!(DomainName::new("app-.roxy").is_err()); // Ends with hyphen
        assert!(DomainName::new("app_name.roxy").is_err()); // Underscore
    }
}
