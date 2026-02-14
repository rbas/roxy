use std::fmt;

use super::domain_name::DomainName;
use crate::infrastructure::certs::WILDCARD_CERT_PREFIX;

/// Value object representing how a domain is matched — either
/// exactly or as a wildcard pattern covering one-level subdomains.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DomainPattern {
    /// Matches only the exact domain (e.g. `myapp.roxy`).
    Exact(DomainName),
    /// Matches the base domain **and** any single-level subdomain
    /// (e.g. `myapp.roxy` + `*.myapp.roxy`).
    Wildcard(DomainName),
}

impl DomainPattern {
    /// Build a `DomainPattern` from a raw domain name string and a
    /// wildcard flag. Validates the domain name and returns the
    /// appropriate variant.
    pub fn from_name(name: &str, wildcard: bool) -> anyhow::Result<Self> {
        let domain = DomainName::new(name)?;
        Ok(if wildcard {
            Self::Wildcard(domain)
        } else {
            Self::Exact(domain)
        })
    }

    /// The underlying base domain regardless of pattern type.
    pub fn base_domain(&self) -> &DomainName {
        match self {
            Self::Exact(d) | Self::Wildcard(d) => d,
        }
    }

    pub fn is_wildcard(&self) -> bool {
        matches!(self, Self::Wildcard(_))
    }

    /// Single source of truth for hostname matching.
    ///
    /// - `Exact` matches only when hostname equals the domain.
    /// - `Wildcard` matches the base domain itself **and**
    ///   any single-label subdomain (e.g. `blog.myapp.roxy`
    ///   but **not** `a.b.myapp.roxy`).
    pub fn matches_hostname(&self, hostname: &str) -> bool {
        match self {
            Self::Exact(domain) => hostname == domain.as_str(),
            Self::Wildcard(base) => {
                let base_str = base.as_str();
                if hostname == base_str {
                    return true;
                }

                let suffix = format!(".{}", base_str);
                if !hostname.ends_with(&suffix) {
                    return false;
                }

                // Only allow a single label before the base domain.
                let prefix = &hostname[..hostname.len() - suffix.len()];
                !prefix.is_empty() && !prefix.contains('.')
            }
        }
    }

    /// Human-readable display pattern (e.g. `*.myapp.roxy`).
    pub fn display_pattern(&self) -> String {
        match self {
            Self::Exact(d) => d.as_str().to_string(),
            Self::Wildcard(d) => format!("*.{}", d.as_str()),
        }
    }

    /// Certificate file stem used for on-disk certificate naming.
    ///
    /// Exact domains use the domain directly (`myapp.roxy`).
    /// Wildcard domains use the `__wildcard__.` prefix
    /// (`__wildcard__.myapp.roxy`).
    pub fn cert_name(&self) -> String {
        match self {
            Self::Exact(d) => d.as_str().to_string(),
            Self::Wildcard(d) => {
                format!("{}{}", WILDCARD_CERT_PREFIX, d.as_str())
            }
        }
    }

    /// Specificity score for "most specific wins" ordering.
    /// Longer base domains are more specific.
    pub fn specificity(&self) -> usize {
        self.base_domain().as_str().len()
    }
}

impl fmt::Display for DomainPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_pattern())
    }
}

impl serde::Serialize for DomainPattern {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.display_pattern())
    }
}

impl<'de> serde::Deserialize<'de> for DomainPattern {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if let Some(base) = s.strip_prefix("*.") {
            let domain = DomainName::new(base).map_err(serde::de::Error::custom)?;
            Ok(DomainPattern::Wildcard(domain))
        } else {
            let domain = DomainName::new(&s).map_err(serde::de::Error::custom)?;
            Ok(DomainPattern::Exact(domain))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exact(name: &str) -> DomainPattern {
        DomainPattern::Exact(DomainName::new(name).unwrap())
    }

    fn wildcard(name: &str) -> DomainPattern {
        DomainPattern::Wildcard(DomainName::new(name).unwrap())
    }

    // --- from_name ---

    #[test]
    fn from_name_creates_exact_pattern() {
        let p = DomainPattern::from_name("myapp.roxy", false).unwrap();
        assert_eq!(p, exact("myapp.roxy"));
    }

    #[test]
    fn from_name_creates_wildcard_pattern() {
        let p = DomainPattern::from_name("myapp.roxy", true).unwrap();
        assert_eq!(p, wildcard("myapp.roxy"));
    }

    #[test]
    fn from_name_rejects_invalid_domain() {
        assert!(DomainPattern::from_name("invalid", false).is_err());
    }

    // --- matches_hostname ---

    #[test]
    fn exact_matches_same_hostname() {
        assert!(exact("myapp.roxy").matches_hostname("myapp.roxy"));
    }

    #[test]
    fn exact_does_not_match_subdomain() {
        assert!(!exact("myapp.roxy").matches_hostname("blog.myapp.roxy"));
    }

    #[test]
    fn exact_does_not_match_different_domain() {
        assert!(!exact("myapp.roxy").matches_hostname("other.roxy"));
    }

    #[test]
    fn wildcard_matches_base_domain() {
        assert!(wildcard("myapp.roxy").matches_hostname("myapp.roxy"));
    }

    #[test]
    fn wildcard_matches_single_level_subdomain() {
        assert!(wildcard("myapp.roxy").matches_hostname("blog.myapp.roxy"));
        assert!(wildcard("myapp.roxy").matches_hostname("api.myapp.roxy"));
    }

    #[test]
    fn wildcard_does_not_match_multi_level_subdomain() {
        assert!(!wildcard("myapp.roxy").matches_hostname("a.b.myapp.roxy"));
    }

    #[test]
    fn wildcard_does_not_match_unrelated_domain() {
        assert!(!wildcard("myapp.roxy").matches_hostname("other.roxy"));
    }

    #[test]
    fn wildcard_does_not_match_suffix_overlap() {
        // "notmyapp.roxy" should NOT match wildcard for "myapp.roxy"
        assert!(!wildcard("myapp.roxy").matches_hostname("notmyapp.roxy"));
    }

    #[test]
    fn wildcard_does_not_match_empty_prefix() {
        // ".myapp.roxy" — empty prefix before the dot
        assert!(!wildcard("myapp.roxy").matches_hostname(".myapp.roxy"));
    }

    // --- display_pattern ---

    #[test]
    fn exact_display_pattern() {
        assert_eq!(exact("myapp.roxy").display_pattern(), "myapp.roxy");
    }

    #[test]
    fn wildcard_display_pattern() {
        assert_eq!(wildcard("myapp.roxy").display_pattern(), "*.myapp.roxy");
    }

    // --- cert_name ---

    #[test]
    fn exact_cert_name() {
        assert_eq!(exact("myapp.roxy").cert_name(), "myapp.roxy");
    }

    #[test]
    fn wildcard_cert_name() {
        assert_eq!(
            wildcard("myapp.roxy").cert_name(),
            "__wildcard__.myapp.roxy"
        );
    }

    // --- specificity ---

    #[test]
    fn longer_base_domain_is_more_specific() {
        let broad = wildcard("myapp.roxy");
        let specific = wildcard("sub.myapp.roxy");
        assert!(specific.specificity() > broad.specificity());
    }

    // --- Display trait ---

    #[test]
    fn display_trait_matches_display_pattern() {
        let p = wildcard("myapp.roxy");
        assert_eq!(format!("{}", p), p.display_pattern());
    }

    // --- is_wildcard ---

    #[test]
    fn is_wildcard_returns_correctly() {
        assert!(!exact("myapp.roxy").is_wildcard());
        assert!(wildcard("myapp.roxy").is_wildcard());
    }

    // --- serde round-trip ---

    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Wrapper {
        pattern: DomainPattern,
    }

    #[test]
    fn serde_roundtrip_exact() {
        let w = Wrapper {
            pattern: exact("myapp.roxy"),
        };
        let toml_str = toml::to_string(&w).unwrap();
        let deserialized: Wrapper = toml::from_str(&toml_str).unwrap();
        assert_eq!(w, deserialized);
    }

    #[test]
    fn serde_roundtrip_wildcard() {
        let w = Wrapper {
            pattern: wildcard("myapp.roxy"),
        };
        let toml_str = toml::to_string(&w).unwrap();
        let deserialized: Wrapper = toml::from_str(&toml_str).unwrap();
        assert_eq!(w, deserialized);
    }

    #[test]
    fn serde_deserializes_wildcard_from_string() {
        let toml_str = r#"pattern = "*.myapp.roxy""#;
        let w: Wrapper = toml::from_str(toml_str).unwrap();
        assert_eq!(w.pattern, wildcard("myapp.roxy"));
    }

    #[test]
    fn serde_deserializes_exact_from_string() {
        let toml_str = r#"pattern = "myapp.roxy""#;
        let w: Wrapper = toml::from_str(toml_str).unwrap();
        assert_eq!(w.pattern, exact("myapp.roxy"));
    }
}
