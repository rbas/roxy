use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use super::router::SharedState;

/// Management commands sent over the Unix socket.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd")]
enum MgmtCommand {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "reload")]
    Reload,
    #[serde(rename = "list")]
    List,
}

/// Response sent back to the management client.
#[derive(Debug, Serialize)]
struct MgmtResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

impl MgmtResponse {
    fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            error: None,
            data: Some(data),
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: Some(msg.into()),
            data: None,
        }
    }
}

/// Parse a management command from a JSON line or a simple string.
fn parse_command(line: &str) -> Option<MgmtCommand> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    // Try JSON first
    if let Ok(cmd) = serde_json::from_str::<MgmtCommand>(line) {
        return Some(cmd);
    }

    // Fall back to plain text commands
    match line {
        "status" => Some(MgmtCommand::Status),
        "reload" => Some(MgmtCommand::Reload),
        "list" => Some(MgmtCommand::List),
        _ => None,
    }
}

/// Daemon port configuration passed to the management socket
/// so status responses include actual configured ports.
#[derive(Debug, Clone, Copy)]
pub struct DaemonPorts {
    pub http: u16,
    pub https: u16,
    pub dns: u16,
}

/// Serve management commands on a Unix domain socket.
///
/// The socket accepts one command per connection (JSON-over-lines protocol).
/// Socket file is cleaned up when the cancellation token fires.
pub async fn serve(
    socket_path: PathBuf,
    state: SharedState,
    reload_tx: mpsc::Sender<()>,
    cancel: CancellationToken,
    ports: DaemonPorts,
) -> anyhow::Result<()> {
    // Remove stale socket file if it exists
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;
    crate::infrastructure::file_security::restrict_key_permissions(&socket_path)
        .map_err(|e| anyhow::anyhow!("Failed to set socket permissions: {e}"))?;
    info!(path = %socket_path.display(), "Management socket listening");

    loop {
        let (stream, _) = tokio::select! {
            _ = cancel.cancelled() => break,
            result = listener.accept() => match result {
                Ok(conn) => conn,
                Err(e) => {
                    error!(error = %e, "Failed to accept management connection");
                    continue;
                }
            },
        };

        let state = state.clone();
        let reload_tx = reload_tx.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            if let Err(e) = reader.read_line(&mut line).await {
                warn!(error = %e, "Failed to read from management socket");
                return;
            }

            let response = match parse_command(&line) {
                Some(MgmtCommand::Status) => {
                    let snapshot = state.load();
                    let count = snapshot.registrations().len();
                    let pid = std::process::id();
                    MgmtResponse::success(serde_json::json!({
                        "pid": pid,
                        "registration_count": count,
                        "http_port": ports.http,
                        "https_port": ports.https,
                        "dns_port": ports.dns,
                    }))
                }
                Some(MgmtCommand::List) => {
                    let snapshot = state.load();
                    let domains: Vec<serde_json::Value> = snapshot
                        .registrations()
                        .iter()
                        .map(|r| {
                            let routes: Vec<serde_json::Value> = r
                                .routes()
                                .iter()
                                .map(|route| {
                                    serde_json::json!({
                                        "path": route.path().as_str(),
                                        "target": route.target().to_string(),
                                    })
                                })
                                .collect();
                            serde_json::json!({
                                "pattern": r.display_pattern(),
                                "source": r.source().to_string(),
                                "https": r.is_https_enabled(),
                                "routes": routes,
                            })
                        })
                        .collect();
                    MgmtResponse::success(serde_json::json!({
                        "domains": domains,
                    }))
                }
                Some(MgmtCommand::Reload) => {
                    if reload_tx.send(()).await.is_ok() {
                        MgmtResponse::success(serde_json::json!({
                            "reloaded": true,
                        }))
                    } else {
                        MgmtResponse::error("Reload channel closed")
                    }
                }
                None => MgmtResponse::error(format!("Unknown command: {}", line.trim())),
            };

            if let Ok(json) = serde_json::to_string(&response) {
                let _ = writer.write_all(json.as_bytes()).await;
                let _ = writer.write_all(b"\n").await;
            }
        });
    }

    // Clean up socket file on shutdown
    let _ = std::fs::remove_file(&socket_path);
    info!("Management socket closed");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::router::RuntimeState;
    use crate::domain::{DomainName, DomainPattern, DomainRegistration, Route};
    use arc_swap::ArcSwap;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::net::UnixStream;

    fn test_reg(domain: &str) -> DomainRegistration {
        let name = DomainName::new(domain).unwrap();
        let pattern = DomainPattern::Exact(name);
        let routes = vec![Route::parse("/=3000").unwrap()];
        DomainRegistration::new(pattern, routes)
    }

    /// Helper: start the management socket and return what tests need to interact with it.
    /// Returns the TempDir to keep it alive for the socket path.
    async fn start_server(
        registrations: Vec<DomainRegistration>,
    ) -> (
        PathBuf,
        CancellationToken,
        mpsc::Receiver<()>,
        tempfile::TempDir,
    ) {
        // Use the conventional short Unix-socket location. Platform temp
        // directories can be long enough to exceed sockaddr_un limits and
        // some macOS sandboxes deny socket creation there.
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let sock = dir.path().join("test.sock");

        let state: SharedState = Arc::new(ArcSwap::from_pointee(RuntimeState::new(registrations)));
        let (reload_tx, reload_rx) = mpsc::channel::<()>(4);
        let cancel = CancellationToken::new();

        let cancel_serve = cancel.clone();
        let sock_clone = sock.clone();
        let test_ports = DaemonPorts {
            http: 80,
            https: 443,
            dns: 1053,
        };
        let handle = tokio::spawn(async move {
            serve(sock_clone, state, reload_tx, cancel_serve, test_ports).await
        });

        tokio::time::timeout(Duration::from_secs(1), async {
            while !sock.exists() {
                if handle.is_finished() {
                    panic!(
                        "management socket task exited before becoming ready: {:?}",
                        handle.await
                    );
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("management socket did not become ready");

        (sock, cancel, reload_rx, dir)
    }

    /// Send a command to the socket, return the parsed JSON response.
    async fn send_command(sock: &PathBuf, cmd: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(cmd.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut buf = String::new();
        let mut reader = BufReader::new(stream);
        reader.read_line(&mut buf).await.unwrap();
        serde_json::from_str(&buf).unwrap()
    }

    // --- parse_command unit tests ---

    #[test]
    fn parse_command_status() {
        let cmd = parse_command(r#"{"cmd":"status"}"#);
        assert!(matches!(cmd, Some(MgmtCommand::Status)));
    }

    #[test]
    fn parse_command_reload() {
        let cmd = parse_command(r#"{"cmd":"reload"}"#);
        assert!(matches!(cmd, Some(MgmtCommand::Reload)));
    }

    #[test]
    fn parse_command_list() {
        let cmd = parse_command(r#"{"cmd":"list"}"#);
        assert!(matches!(cmd, Some(MgmtCommand::List)));
    }

    #[test]
    fn parse_command_simple_strings() {
        assert!(matches!(parse_command("status"), Some(MgmtCommand::Status)));
        assert!(matches!(parse_command("reload"), Some(MgmtCommand::Reload)));
        assert!(matches!(parse_command("list"), Some(MgmtCommand::List)));
        // With whitespace
        assert!(matches!(
            parse_command("  status  "),
            Some(MgmtCommand::Status)
        ));
    }

    #[test]
    fn parse_command_unknown() {
        assert!(parse_command("foobar").is_none());
        assert!(parse_command("").is_none());
        assert!(parse_command(r#"{"cmd":"unknown"}"#).is_none());
    }

    // --- serve() integration tests ---

    #[tokio::test]
    async fn serve_status_returns_pid_and_count() {
        let (sock, cancel, _rx, _dir) =
            start_server(vec![test_reg("app.roxy"), test_reg("api.roxy")]).await;

        let resp = send_command(&sock, r#"{"cmd":"status"}"#).await;
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["registration_count"], 2);
        assert!(resp["data"]["pid"].as_u64().unwrap() > 0);

        cancel.cancel();
    }

    #[tokio::test]
    async fn serve_list_returns_domain_details() {
        let (sock, cancel, _rx, _dir) = start_server(vec![test_reg("app.roxy")]).await;

        let resp = send_command(&sock, r#"{"cmd":"list"}"#).await;
        assert_eq!(resp["ok"], true);
        let domains = resp["data"]["domains"].as_array().unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0]["pattern"], "app.roxy");
        assert_eq!(domains[0]["source"], "config");
        assert_eq!(domains[0]["https"], true);
        let routes = domains[0]["routes"].as_array().unwrap();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0]["path"], "/");

        cancel.cancel();
    }

    #[tokio::test]
    async fn serve_reload_pushes_to_channel() {
        let (sock, cancel, mut rx, _dir) = start_server(vec![test_reg("old.roxy")]).await;

        let resp = send_command(&sock, r#"{"cmd":"reload"}"#).await;
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["reloaded"], true);

        // Verify the reload channel received a nudge
        tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out")
            .expect("channel closed");

        cancel.cancel();
    }

    #[tokio::test]
    async fn serve_unknown_command_returns_error() {
        let (sock, cancel, _rx, _dir) = start_server(vec![]).await;

        let resp = send_command(&sock, "foobar").await;
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("Unknown command"));

        cancel.cancel();
    }

    #[tokio::test]
    async fn serve_plain_text_status() {
        let (sock, cancel, _rx, _dir) = start_server(vec![test_reg("app.roxy")]).await;

        let resp = send_command(&sock, "status").await;
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["registration_count"], 1);

        cancel.cancel();
    }

    #[tokio::test]
    async fn serve_cleans_up_socket_on_cancel() {
        let (sock, cancel, _rx, _dir) = start_server(vec![]).await;
        assert!(sock.exists());

        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(1), async {
            while sock.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("management socket was not removed");
        assert!(!sock.exists());
    }
}
