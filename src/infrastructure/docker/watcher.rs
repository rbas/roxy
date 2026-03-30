use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use bollard::Docker;
use bollard::models::EventMessageTypeEnum;
use bollard::query_parameters::EventsOptions;
use bollard::query_parameters::ListContainersOptions;
use futures_util::StreamExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::discovery::{ContainerInfo, DiscoveryResult, evaluate_container};
use super::network;
use crate::domain::DomainRegistration;

/// Watch Docker events and maintain an up-to-date list of registrations.
///
/// On each container start/stop event, performs a full reconciliation:
/// lists all running containers, evaluates each one, and updates the
/// shared state. Then sends a `()` nudge to trigger server reload.
pub async fn watch(
    docker: Docker,
    state: Arc<RwLock<Vec<DomainRegistration>>>,
    nudge_tx: mpsc::Sender<()>,
    cancel: CancellationToken,
) {
    // Initial reconciliation on startup
    if let Err(e) = reconcile(&docker, &state, &nudge_tx).await {
        warn!(error = %e, "Initial Docker reconciliation failed");
    }

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                info!("Docker watcher shutting down");
                break;
            }
            result = watch_events(&docker, &state, &nudge_tx, &cancel) => {
                match result {
                    Ok(()) => break, // cancelled
                    Err(e) => {
                        warn!(error = %e, "Docker event stream error, reconnecting in 5s");
                        tokio::select! {
                            _ = cancel.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                        }
                    }
                }
            }
        }
    }
}

/// How long to wait after the last Docker event before reconciling.
/// This prevents a burst of events (e.g., `docker compose up` starting
/// 10 services) from triggering 10 separate full reconciliations.
const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);

/// Subscribe to Docker events and reconcile on container lifecycle events.
///
/// Uses a debounce timer: each qualifying event resets the timer, and
/// reconciliation only runs once the timer expires without new events.
async fn watch_events(
    docker: &Docker,
    state: &Arc<RwLock<Vec<DomainRegistration>>>,
    nudge_tx: &mpsc::Sender<()>,
    cancel: &CancellationToken,
) -> anyhow::Result<()> {
    let mut filters = HashMap::new();
    filters.insert("type".to_string(), vec!["container".to_string()]);
    filters.insert(
        "event".to_string(),
        vec![
            "start".to_string(),
            "stop".to_string(),
            "die".to_string(),
            "destroy".to_string(),
        ],
    );

    let options = EventsOptions {
        filters: Some(filters),
        ..Default::default()
    };

    let mut stream = docker.events(Some(options));
    let mut debounce = std::pin::pin!(tokio::time::sleep(DEBOUNCE_DURATION));
    let mut pending = false;

    info!("Docker event stream connected");

    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = &mut debounce, if pending => {
                pending = false;
                if let Err(e) = reconcile(docker, state, nudge_tx).await {
                    warn!(error = %e, "Docker reconciliation failed after event");
                }
            }
            event = stream.next() => {
                match event {
                    Some(Ok(ev)) => {
                        let action = ev.action.as_deref().unwrap_or("unknown");
                        let actor_id = ev
                            .actor
                            .as_ref()
                            .and_then(|a| a.id.as_deref())
                            .unwrap_or("unknown");

                        // Only debounce on container events
                        if ev.typ == Some(EventMessageTypeEnum::CONTAINER) {
                            debug!(
                                action,
                                container = actor_id,
                                "Docker container event, scheduling reconciliation"
                            );
                            // Reset the debounce timer
                            debounce.as_mut().reset(tokio::time::Instant::now() + DEBOUNCE_DURATION);
                            pending = true;
                        }
                    }
                    Some(Err(e)) => {
                        return Err(anyhow::anyhow!(e));
                    }
                    None => {
                        return Err(anyhow::anyhow!("Docker event stream ended"));
                    }
                }
            }
        }
    }
}

/// Full reconciliation: list all running containers, evaluate each,
/// and replace the shared state.
async fn reconcile(
    docker: &Docker,
    state: &Arc<RwLock<Vec<DomainRegistration>>>,
    nudge_tx: &mpsc::Sender<()>,
) -> anyhow::Result<()> {
    let containers = docker
        .list_containers(Some(ListContainersOptions {
            all: false, // only running
            ..Default::default()
        }))
        .await?;

    let mut registrations = Vec::new();

    for container in &containers {
        let id = match &container.id {
            Some(id) => id,
            None => continue,
        };

        // Inspect the container for full details
        let inspect = match docker.inspect_container(id, None).await {
            Ok(info) => info,
            Err(e) => {
                warn!(container = %id, error = %e, "Failed to inspect container");
                continue;
            }
        };

        let name = inspect
            .name
            .as_deref()
            .unwrap_or("")
            .trim_start_matches('/')
            .to_string();

        let labels = inspect
            .config
            .as_ref()
            .and_then(|c| c.labels.clone())
            .unwrap_or_default();

        let exposed_ports: Vec<u16> = inspect
            .config
            .as_ref()
            .and_then(|c| c.exposed_ports.as_ref())
            .map(|ports| network::get_exposed_ports(ports))
            .unwrap_or_default();

        let host_port_mappings = network::get_host_port_mappings(
            &inspect
                .network_settings
                .as_ref()
                .and_then(|ns| ns.ports.clone()),
        );

        let info = ContainerInfo {
            id: id.clone(),
            name,
            labels,
            exposed_ports,
            host_port_mappings,
        };

        match evaluate_container(&info) {
            DiscoveryResult::Register(reg) => {
                debug!(
                    domain = %reg.display_pattern(),
                    container = %info.name,
                    "Docker container registered"
                );
                registrations.push(reg);
            }
            DiscoveryResult::Skip(reason) => {
                debug!(container = %info.name, reason, "Docker container skipped");
            }
        }
    }

    let count = registrations.len();

    // Compare old vs new to find adds/removes before swapping state
    let (added_count, removed_count) = {
        let guard = state
            .read()
            .map_err(|e| anyhow::anyhow!("Docker state lock poisoned: {e}"))?;

        let old_domains: HashSet<String> = guard.iter().map(|r| r.display_pattern()).collect();
        let new_domains: HashSet<String> =
            registrations.iter().map(|r| r.display_pattern()).collect();

        for pattern in new_domains.difference(&old_domains) {
            if let Some(reg) = registrations
                .iter()
                .find(|r| &r.display_pattern() == pattern)
            {
                let target = reg
                    .routes()
                    .first()
                    .map(|r| r.target().to_string())
                    .unwrap_or_default();
                info!(domain = %pattern, target = %target, "Docker domain added");
            }
        }
        for pattern in old_domains.difference(&new_domains) {
            info!(domain = %pattern, "Docker domain removed");
        }

        (
            new_domains.difference(&old_domains).count(),
            old_domains.difference(&new_domains).count(),
        )
    };

    // Update shared state
    {
        let mut guard = state
            .write()
            .map_err(|e| anyhow::anyhow!("Docker state lock poisoned: {e}"))?;
        *guard = registrations;
    }

    if added_count > 0 || removed_count > 0 {
        info!(
            added = added_count,
            removed = removed_count,
            total = count,
            "Docker reconciliation complete"
        );
        // Nudge the server to reload only when registrations changed
        let _ = nudge_tx.send(()).await;
    }

    Ok(())
}
