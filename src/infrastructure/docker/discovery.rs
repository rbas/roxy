use std::collections::HashMap;

use tracing::warn;

use crate::domain::value_objects::port::Port;
use crate::domain::{
    DomainName, DomainPattern, DomainRegistration, ProxyTarget, RegistrationSource, Route,
};

/// Information extracted from a Docker container for registration decisions.
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub labels: HashMap<String, String>,
    /// Ports exposed by the container (internal ports, not host mappings).
    pub exposed_ports: Vec<u16>,
    /// Host port mappings (container_port -> host_port) from `-p` / `ports:`.
    pub host_port_mappings: HashMap<u16, u16>,
}

/// Result of evaluating a container for registration.
#[derive(Debug)]
pub enum DiscoveryResult {
    /// Container qualifies; here's its registration.
    Register(DomainRegistration),
    /// Container was explicitly opted out or doesn't qualify.
    Skip(String),
}

/// Evaluate a container and decide whether to register it.
///
/// Container qualification rules:
/// 1. Has `roxy.enable=false` label --> skip (explicit opt-out)
/// 2. Has `roxy.enable=true` label --> register (explicit opt-in)
/// 3. Has compose labels AND at least one exposed port --> register
/// 4. Otherwise --> skip
///
/// The proxy target is always `127.0.0.1:{host_port}` using the
/// container's published host port mapping. Containers without
/// published ports cannot be proxied.
pub fn evaluate_container(info: &ContainerInfo) -> DiscoveryResult {
    // Check explicit opt-out
    if info.labels.get("roxy.enable").is_some_and(|v| v == "false") {
        return DiscoveryResult::Skip("explicitly disabled via roxy.enable=false".into());
    }

    let explicit_opt_in = info.labels.get("roxy.enable").is_some_and(|v| v == "true");

    // Determine domain name
    let domain = match resolve_domain(info) {
        Some(d) => d,
        None => {
            if explicit_opt_in {
                return DiscoveryResult::Skip(format!(
                    "container {} has roxy.enable=true but no domain could be determined \
                     (set roxy.domain or use docker compose)",
                    info.name
                ));
            }
            return DiscoveryResult::Skip("no compose labels and no roxy.domain".into());
        }
    };

    // Check if container qualifies (explicit opt-in or compose with ports)
    if !explicit_opt_in {
        let has_compose_labels = info.labels.contains_key("com.docker.compose.project")
            && info.labels.contains_key("com.docker.compose.service");

        if !has_compose_labels {
            return DiscoveryResult::Skip("no compose labels and roxy.enable not set".into());
        }

        if info.exposed_ports.is_empty() {
            return DiscoveryResult::Skip(format!(
                "compose service {} has no exposed ports",
                info.name
            ));
        }
    }

    // Resolve container port
    let port = match resolve_port(info) {
        Some(p) => p,
        None => {
            return DiscoveryResult::Skip(format!(
                "container {} has multiple exposed ports ({:?}), set roxy.port to choose one",
                info.name, info.exposed_ports
            ));
        }
    };

    // Look up the host port mapping for this container port.
    let host_port = match info.host_port_mappings.get(&port.value()) {
        Some(&hp) => hp,
        None => {
            return DiscoveryResult::Skip(format!(
                "container {} has no published host port for container port {}. \
                 Add a `ports:` mapping in docker-compose.yml (e.g., \"8080:{}\")",
                info.name,
                port.value(),
                port.value(),
            ));
        }
    };

    let target_port = match Port::any(host_port) {
        Ok(p) => p,
        Err(e) => {
            return DiscoveryResult::Skip(format!(
                "container {} has invalid host port {host_port}: {e}",
                info.name
            ));
        }
    };

    let target = ProxyTarget::localhost(target_port);
    let root_path = match crate::domain::PathPrefix::new("/") {
        Ok(p) => p,
        Err(e) => {
            return DiscoveryResult::Skip(format!("bug: root path is invalid: {e}"));
        }
    };
    let route = Route::new(root_path, crate::domain::RouteTarget::Proxy(target));

    // Determine if wildcard
    let is_wildcard = info
        .labels
        .get("roxy.wildcard")
        .is_some_and(|v| v == "true");

    let pattern = if is_wildcard {
        DomainPattern::Wildcard(domain)
    } else {
        DomainPattern::Exact(domain)
    };

    let mut reg =
        DomainRegistration::with_source(pattern, vec![route], RegistrationSource::External);
    reg.enable_https();

    DiscoveryResult::Register(reg)
}

/// Resolve the domain name for a container.
///
/// Priority:
/// 1. `roxy.domain` label (explicit override)
/// 2. `{service}.{project}.roxy` from compose labels
fn resolve_domain(info: &ContainerInfo) -> Option<DomainName> {
    // Check for explicit domain override
    if let Some(domain_str) = info.labels.get("roxy.domain") {
        return match DomainName::new(domain_str) {
            Ok(d) => Some(d),
            Err(e) => {
                warn!(
                    container = %info.name,
                    domain = %domain_str,
                    error = %e,
                    "Invalid roxy.domain label"
                );
                None
            }
        };
    }

    // Build from compose labels
    let project = info.labels.get("com.docker.compose.project")?;
    let service = info.labels.get("com.docker.compose.service")?;

    let domain_str = format!("{service}.{project}.roxy");
    match DomainName::new(&domain_str) {
        Ok(d) => Some(d),
        Err(e) => {
            warn!(
                container = %info.name,
                domain = %domain_str,
                error = %e,
                "Could not create valid domain from compose labels"
            );
            None
        }
    }
}

/// Resolve the target port for a container.
///
/// Priority:
/// 1. `roxy.port` label (explicit)
/// 2. Single exposed port (auto-detect)
/// 3. None (ambiguous — multiple ports without label)
fn resolve_port(info: &ContainerInfo) -> Option<Port> {
    // Check for explicit port label
    if let Some(port_str) = info.labels.get("roxy.port") {
        return match port_str.parse::<u16>() {
            Ok(p) => match Port::any(p) {
                Ok(port) => Some(port),
                Err(e) => {
                    warn!(
                        container = %info.name,
                        port = %port_str,
                        error = %e,
                        "Invalid roxy.port label"
                    );
                    None
                }
            },
            Err(e) => {
                warn!(
                    container = %info.name,
                    port = %port_str,
                    error = %e,
                    "Cannot parse roxy.port label as number"
                );
                None
            }
        };
    }

    // Auto-detect from exposed ports
    match info.exposed_ports.len() {
        0 => None,
        1 => Port::any(info.exposed_ports[0]).ok(),
        _ => None, // Multiple ports — ambiguous
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(
        labels: Vec<(&str, &str)>,
        exposed_ports: Vec<u16>,
        host_port_mappings: HashMap<u16, u16>,
    ) -> ContainerInfo {
        ContainerInfo {
            id: "abc123".into(),
            name: "test-container".into(),
            labels: labels
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            exposed_ports,
            host_port_mappings,
        }
    }

    // --- evaluate_container ---

    #[test]
    fn explicit_opt_out_skips() {
        let info = make_info(
            vec![
                ("roxy.enable", "false"),
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
            ],
            vec![3000],
            HashMap::from([(3000, 8080)]),
        );
        assert!(matches!(
            evaluate_container(&info),
            DiscoveryResult::Skip(_)
        ));
    }

    #[test]
    fn explicit_opt_in_with_domain_and_port() {
        let info = make_info(
            vec![
                ("roxy.enable", "true"),
                ("roxy.domain", "custom.roxy"),
                ("roxy.port", "8080"),
            ],
            vec![],
            HashMap::from([(8080, 9090)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                assert_eq!(reg.domain().as_str(), "custom.roxy");
                let route = &reg.routes()[0];
                assert_eq!(route.target().to_string(), "127.0.0.1:9090");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    #[test]
    fn compose_service_auto_discovery() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
            ],
            vec![3000],
            HashMap::from([(3000, 8080)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                assert_eq!(reg.domain().as_str(), "web.myproject.roxy");
                assert!(reg.is_https_enabled());
                let route = &reg.routes()[0];
                assert_eq!(route.target().to_string(), "127.0.0.1:8080");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    #[test]
    fn compose_service_without_ports_skips() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "db"),
            ],
            vec![],
            HashMap::new(),
        );
        assert!(matches!(
            evaluate_container(&info),
            DiscoveryResult::Skip(_)
        ));
    }

    #[test]
    fn no_labels_skips() {
        let info = make_info(vec![], vec![3000], HashMap::from([(3000, 8080)]));
        assert!(matches!(
            evaluate_container(&info),
            DiscoveryResult::Skip(_)
        ));
    }

    // --- Domain resolution ---

    #[test]
    fn roxy_domain_label_overrides_compose() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
                ("roxy.domain", "custom.roxy"),
            ],
            vec![3000],
            HashMap::from([(3000, 8080)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                assert_eq!(reg.domain().as_str(), "custom.roxy");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    // --- Port resolution ---

    #[test]
    fn roxy_port_label_overrides_exposed() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
                ("roxy.port", "8080"),
            ],
            vec![3000, 4000],
            HashMap::from([(8080, 9090)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                let route = &reg.routes()[0];
                assert_eq!(route.target().to_string(), "127.0.0.1:9090");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    #[test]
    fn single_exposed_port_auto_detected() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
            ],
            vec![3000],
            HashMap::from([(3000, 8080)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                let route = &reg.routes()[0];
                assert_eq!(route.target().to_string(), "127.0.0.1:8080");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    #[test]
    fn multiple_ports_without_label_skips() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
            ],
            vec![3000, 4000],
            HashMap::from([(3000, 8080), (4000, 9090)]),
        );
        assert!(matches!(
            evaluate_container(&info),
            DiscoveryResult::Skip(_)
        ));
    }

    #[test]
    fn privileged_container_port_with_host_mapping() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "nginx"),
            ],
            vec![80],
            HashMap::from([(80, 8080)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                let route = &reg.routes()[0];
                assert_eq!(route.target().to_string(), "127.0.0.1:8080");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    // --- Wildcard support ---

    #[test]
    fn wildcard_label_creates_wildcard_pattern() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
                ("roxy.wildcard", "true"),
            ],
            vec![3000],
            HashMap::from([(3000, 8080)]),
        );
        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                assert!(reg.is_wildcard());
                assert_eq!(reg.display_pattern(), "*.web.myproject.roxy");
            }
            DiscoveryResult::Skip(reason) => panic!("Expected Register, got Skip: {reason}"),
        }
    }

    // --- Missing host port mapping ---

    #[test]
    fn no_host_port_mapping_skips_with_helpful_message() {
        let info = make_info(
            vec![
                ("com.docker.compose.project", "myproject"),
                ("com.docker.compose.service", "web"),
            ],
            vec![3000],
            HashMap::new(), // No published ports
        );
        match evaluate_container(&info) {
            DiscoveryResult::Skip(reason) => {
                assert!(reason.contains("no published host port"));
                assert!(reason.contains("ports:"));
            }
            DiscoveryResult::Register(_) => panic!("Expected Skip"),
        }
    }
}
