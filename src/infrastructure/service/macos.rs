use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::RuntimeUser;
use crate::config::DaemonConfig;
use crate::infrastructure::paths::RoxyPaths;

const LABEL: &str = "com.roxy.proxy";
const PLIST_PATH: &str = "/Library/LaunchDaemons/com.roxy.proxy.plist";

pub fn install(
    executable: &Path,
    config_path: &Path,
    _paths: &RoxyPaths,
    daemon: &DaemonConfig,
    user: &RuntimeUser,
) -> Result<()> {
    remove_legacy_services()?;

    let plist = render_plist(executable, config_path, daemon, user);
    fs::write(PLIST_PATH, plist).context("Failed to write Roxy launchd service")?;

    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{LABEL}")])
        .output();
    run_launchctl(["bootstrap", "system", PLIST_PATH])?;
    run_launchctl(["enable", &format!("system/{LABEL}")])?;
    run_launchctl(["kickstart", "-k", &format!("system/{LABEL}")])?;
    Ok(())
}

fn remove_legacy_services() -> Result<()> {
    for (label, path) in [
        ("cz.rbas.roxy", "/Library/LaunchDaemons/cz.rbas.roxy.plist"),
        (
            "homebrew.mxcl.roxy",
            "/Library/LaunchDaemons/homebrew.mxcl.roxy.plist",
        ),
    ] {
        let _ = Command::new("launchctl")
            .args(["bootout", &format!("system/{label}")])
            .output();
        if Path::new(path).exists() {
            fs::remove_file(path).with_context(|| format!("Failed to remove legacy {path}"))?;
        }
    }
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("system/{LABEL}")])
        .output();
    if Path::new(PLIST_PATH).exists() {
        fs::remove_file(PLIST_PATH).context("Failed to remove Roxy launchd service")?;
    }
    remove_legacy_services()?;
    Ok(())
}

pub fn is_installed() -> bool {
    Path::new(PLIST_PATH).exists()
}

fn render_plist(
    executable: &Path,
    config_path: &Path,
    daemon: &DaemonConfig,
    user: &RuntimeUser,
) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--config</string>
    <string>{}</string>
    <string>start</string>
    <string>--foreground</string>
  </array>
  <key>UserName</key><string>{}</string>
  <key>RunAtLoad</key><true/>
  <key>Sockets</key>
  <dict>
    <key>Http</key>
    <dict>
      <key>SockNodeName</key><string>0.0.0.0</string>
      <key>SockServiceName</key><string>{}</string>
      <key>SockType</key><string>stream</string>
      <key>SockFamily</key><string>IPv4</string>
    </dict>
    <key>Https</key>
    <dict>
      <key>SockNodeName</key><string>0.0.0.0</string>
      <key>SockServiceName</key><string>{}</string>
      <key>SockType</key><string>stream</string>
      <key>SockFamily</key><string>IPv4</string>
    </dict>
  </dict>
</dict>
</plist>
"#,
        xml_escape(&executable.display().to_string()),
        xml_escape(&config_path.display().to_string()),
        xml_escape(&user.name),
        daemon.http_port,
        daemon.https_port,
    )
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn run_launchctl<const N: usize>(args: [&str; N]) -> Result<()> {
    let output = Command::new("launchctl")
        .args(args)
        .output()
        .context("Failed to run launchctl")?;
    if !output.status.success() {
        bail!(
            "launchctl failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_contains_unprivileged_user_and_sockets() {
        let plist = render_plist(
            Path::new("/usr/local/bin/roxy"),
            Path::new("/Users/dev/Library/Application Support/Roxy/config.toml"),
            &DaemonConfig::default(),
            &RuntimeUser {
                name: "dev".into(),
                uid: 501,
                gid: 20,
                home: "/Users/dev".into(),
            },
        );
        assert!(plist.contains("<key>UserName</key><string>dev</string>"));
        assert!(plist.contains("<key>Http</key>"));
        assert!(plist.contains("<string>443</string>"));
    }
}
