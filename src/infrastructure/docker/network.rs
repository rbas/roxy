use std::collections::HashMap;

/// Extract host port mappings from a container's network settings.
///
/// Returns a map of container_port -> host_port for ports published
/// to the host (via `-p` or `ports:` in compose).
///
/// Roxy runs on the host and proxies to containers via their published
/// ports. Container-to-container resolution uses `extra_hosts` pointing
/// at `host-gateway` so traffic flows through Roxy on the host.
pub fn get_host_port_mappings(
    ports: &Option<HashMap<String, Option<Vec<bollard::models::PortBinding>>>>,
) -> HashMap<u16, u16> {
    let mut mappings = HashMap::new();
    let Some(ports) = ports else {
        return mappings;
    };

    for (container_port_key, bindings) in ports {
        // Key format: "80/tcp"
        let container_port = match container_port_key.split('/').next() {
            Some(p) => match p.parse::<u16>() {
                Ok(port) => port,
                Err(_) => continue,
            },
            None => continue,
        };

        let Some(bindings) = bindings else { continue };

        for binding in bindings {
            if let Some(host_port_str) = &binding.host_port
                && let Ok(host_port) = host_port_str.parse::<u16>()
            {
                mappings.insert(container_port, host_port);
                break; // Use first binding
            }
        }
    }

    mappings
}

/// Get exposed ports from a container's inspect response.
///
/// Extracts port numbers from the image's ExposedPorts config
/// (e.g., {"3000/tcp": {}} -> [3000]).
pub fn get_exposed_ports(exposed_ports: &HashMap<String, HashMap<(), ()>>) -> Vec<u16> {
    exposed_ports
        .keys()
        .filter_map(|key| {
            // Format is "port/proto" e.g., "3000/tcp"
            let port_str = key.split('/').next()?;
            port_str.parse::<u16>().ok()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exposed_ports() {
        let mut ports = HashMap::new();
        ports.insert("3000/tcp".to_string(), HashMap::new());
        ports.insert("8080/tcp".to_string(), HashMap::new());

        let mut result = get_exposed_ports(&ports);
        result.sort();
        assert_eq!(result, vec![3000, 8080]);
    }

    #[test]
    fn parse_exposed_ports_empty() {
        let ports = HashMap::new();
        let result = get_exposed_ports(&ports);
        assert!(result.is_empty());
    }

    #[test]
    fn parse_exposed_ports_udp() {
        let mut ports = HashMap::new();
        ports.insert("53/udp".to_string(), HashMap::new());

        let result = get_exposed_ports(&ports);
        assert_eq!(result, vec![53]);
    }

    // --- Host port mappings ---

    #[test]
    fn host_port_mappings_extracted() {
        use bollard::models::PortBinding;

        let mut ports = HashMap::new();
        ports.insert(
            "80/tcp".to_string(),
            Some(vec![PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some("8081".to_string()),
            }]),
        );

        let mappings = get_host_port_mappings(&Some(ports));
        assert_eq!(mappings.get(&80), Some(&8081));
    }

    #[test]
    fn host_port_mappings_none_bindings() {
        let mut ports = HashMap::new();
        ports.insert("80/tcp".to_string(), None);

        let mappings = get_host_port_mappings(&Some(ports));
        assert!(mappings.is_empty());
    }

    #[test]
    fn host_port_mappings_none_ports() {
        let mappings = get_host_port_mappings(&None);
        assert!(mappings.is_empty());
    }
}
