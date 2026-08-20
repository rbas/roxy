# Roxy

Roxy is a local development proxy for macOS and Linux that
gives your projects custom `.roxy` domains with automatic
HTTPS. It ships as a single binary with no external
dependencies. Register a domain, point it at a port or
directory, and open `https://myapp.roxy` in your browser.

> **Linux users:** See [linux.md](linux.md) for
> platform-specific setup details and troubleshooting.

## Quick Start

```bash
# One-time privileged setup — installs the CA, DNS,
# and a system service that runs Roxy as your user
sudo roxy install

# Register a domain that proxies to localhost:3000
roxy register myapp.roxy --route "/=3000"

# Open in browser
open https://myapp.roxy        # macOS
xdg-open https://myapp.roxy   # Linux
```

## Commands Reference

| Command                            | Description            |
| ---------------------------------- | ---------------------- |
| `sudo roxy install`                | Initial setup          |
| `sudo roxy uninstall [--force]`    | Full cleanup           |
| `roxy register <domain> ...`       | Register domain        |
| `roxy register --wildcard ..`      | Register wildcard      |
| `roxy unregister <domain>`         | Remove domain          |
| `roxy list`                        | Show all domains       |
| `roxy route add ...`               | Add route to domain    |
| `roxy route remove ...`            | Remove route           |
| `roxy route list <domain>`         | List routes for domain |
| `roxy start`                       | Start daemon           |
| `roxy stop`                        | Stop daemon            |
| `roxy restart`                     | Restart daemon         |
| `roxy reload`                      | Reload configuration   |
| `roxy status`                      | Show daemon status     |
| `roxy logs [-n N] [-f]`            | View or follow logs    |
| `roxy completions <shell>`         | Generate completions   |

**Note:** Only `install` and `uninstall` modify system
configuration and require `sudo`. Registration, routing,
logs, reloads, and daemon control run as your user.

## Route Targets

Routes map a URL path prefix to a target. The format
is `PATH=TARGET`.

**Port** — proxy to `127.0.0.1` on the given port:

```bash
roxy register app.roxy --route "/=3000"
```

**Host and port** — proxy to a specific address:

```bash
roxy register app.roxy --route "/=192.168.1.50:3000"
```

**Directory** — serve static files from disk:

```bash
roxy register app.roxy --route "/=/var/www/html"
```

**Multiple routes** — combine targets on one domain.
The longest matching prefix wins:

```bash
roxy register app.roxy \
  --route "/=3000" \
  --route "/api=3001"
```

You can also manage routes after registration:

```bash
roxy route add app.roxy /webhooks 9000
roxy route remove app.roxy /webhooks
roxy route list app.roxy
```

## Reverse Proxy Behavior

When forwarding requests to a backend service, Roxy
sets standard proxy headers and strips hop-by-hop headers
per [RFC 7230 §6.1][rfc7230].

[rfc7230]: https://www.rfc-editor.org/rfc/rfc7230#section-6.1

### Forwarding Headers

Roxy adds these headers to every proxied request so your
backend knows the original client details:

| Header | Value |
| ------ | ----- |
| `X-Forwarded-Host` | Original `Host` header from the client |
| `X-Forwarded-Proto` | `http` or `https` |
| `X-Forwarded-For` | Client IP (appended to existing chain) |

Most frameworks use these automatically. For example,
Django reads `X-Forwarded-Proto` to decide whether to
generate `https://` URLs, and Rails uses
`X-Forwarded-Host` for routing.

### Hop-by-Hop Header Stripping

Roxy removes the following hop-by-hop headers from both
the forwarded request and the backend response:

`Connection`, `Keep-Alive`, `Proxy-Authenticate`,
`Proxy-Authorization`, `TE`, `Trailer`,
`Transfer-Encoding`, `Upgrade`, plus any headers
listed in the `Connection` header value.

These headers describe the connection between two
adjacent nodes (client↔proxy or proxy↔backend) and
must not leak across hops.

### Debugging Proxy Headers

Set `daemon.log_level = "debug"` in the user configuration,
restart, and follow the log to see the forwarding headers Roxy
sets on each request (see
[Logging and Verbosity](#logging-and-verbosity)):

```bash
roxy restart
roxy logs -f
```

```text
DEBUG Forwarding headers set x_forwarded_host=myapp.roxy
  x_forwarded_proto=https x_forwarded_for=127.0.0.1
DEBUG Proxying HTTP request target=127.0.0.1:3000
```

## Wildcard Subdomains

Register a domain with `--wildcard` to match the base
domain **and** any single-level subdomain. Roxy generates
an exact certificate in memory for each requested hostname,
so every matching subdomain gets trusted HTTPS automatically.

```bash
roxy register myapp.roxy --wildcard --route "/=3000"
```

This single registration handles all of these:

| URL | Matches? |
| --- | -------- |
| `https://myapp.roxy` | yes |
| `https://blog.myapp.roxy` | yes |
| `https://api.myapp.roxy` | yes |
| `https://a.b.myapp.roxy` | no (multi-level) |
| `https://other.roxy` | no (different domain) |

### Combining Exact and Wildcard

You can register both an exact domain and a wildcard
for the same base domain. The exact registration takes
priority when both match:

```bash
# Exact: dedicated routes for the base domain
roxy register myapp.roxy --route "/=3000"

# Wildcard: catch-all for subdomains
roxy register myapp.roxy --wildcard --route "/=4000"

# myapp.roxy        → port 3000 (exact wins)
# blog.myapp.roxy   → port 4000 (wildcard)
# api.myapp.roxy    → port 4000 (wildcard)
```

### Managing Wildcard Routes

Use `--wildcard` with `route` subcommands to manage
routes on a wildcard registration:

```bash
roxy route add --wildcard myapp.roxy /api 3001
roxy route remove --wildcard myapp.roxy /api
roxy route list --wildcard myapp.roxy
```

### Unregistering a Wildcard

```bash
roxy unregister --wildcard myapp.roxy
```

This removes the wildcard routing registration. Any exact
registration for the same domain is left untouched.

### Configuration

Wildcard registrations are stored in `config.toml`
with a `*.` prefix on the domain:

```toml
[domains.wildcard-myapp-roxy]
domain = "*.myapp.roxy"
https_enabled = true

[[domains.wildcard-myapp-roxy.routes]]
path = "/"
target = "127.0.0.1:3000"
```

## Static File Serving

When serving static files from a directory, Roxy provides:

**Index file support** — if a directory contains `index.html`,
it's served automatically:

```bash
# Directory structure:
# /var/www/mysite/
# ├── index.html
# ├── about/
# │   └── index.html
# └── assets/
#     └── style.css

roxy register site.roxy --route "/=/var/www/mysite"

# Behavior:
# https://site.roxy          → serves /var/www/mysite/index.html
# https://site.roxy/about/   → serves /var/www/mysite/about/index.html
# https://site.roxy/assets/  → shows file browser (no index.html)
```

**File browser** — directories without `index.html` display an
automatic directory listing, making it easy to browse files and
navigate subdirectories

## Files and Directories

Roxy keeps mutable state in the developer account that ran
`sudo roxy install`:

```text
macOS
~/Library/Application Support/Roxy/config.toml
~/Library/Application Support/Roxy/ca.{key,crt}
~/Library/Caches/Roxy/{roxy.pid,roxy.sock}
~/Library/Logs/Roxy/roxy.log

Linux
~/.config/roxy/config.toml
~/.local/share/roxy/ca.{key,crt}
~/.local/state/roxy/{roxy.log,run/}
```

Leaf certificates are generated from TLS SNI and cached only
in daemon memory. No per-domain private keys are stored.

DNS configuration (created by `roxy install`):

**macOS:**

```text
/etc/resolver/roxy
```

**Linux (systemd-resolved):**

```text
/etc/systemd/resolved.conf.d/roxy.conf
```

This tells the system to resolve all `*.roxy` domains
through the local DNS server.

All paths are configurable via the `[paths]` section in
`config.toml` (see [Configuration](#configuration)).

### Upgrading from the root-daemon layout

Run `sudo roxy install` once after upgrading. Roxy imports
registrations and the Root CA from `/etc/roxy`, stops the old
root daemon/service, writes the new user-owned configuration,
and installs socket activation. The legacy `/etc/roxy`
directory is left in place as a migration backup.

## Auto-Start and Privileged Ports

`sudo roxy install` configures this automatically. On macOS,
launchd owns ports 80 and 443. On Linux, systemd socket units
own them. The operating system passes those open listeners to
Roxy, whose daemon process runs as your developer account.

This is why routine commands do not need root privileges.
There is no separate `brew services` or hand-written systemd
unit to install.

## Daemon: Foreground vs Background

**Managed service** (default after installation) — launchd or
systemd runs Roxy as your user and writes to the user log path:

```bash
roxy start
```

**Foreground** — stays in the terminal, logs to stdout,
and stops with Ctrl+C. This is intended for development before
system installation or with a custom config using unprivileged
ports; the installed socket service already owns ports 80/443:

```bash
roxy --config ./roxy-dev.toml start --foreground
```

## Logging and Verbosity

View logs:

```bash
roxy logs              # last 50 lines
roxy logs -n 100       # last 100 lines
roxy logs -f           # follow (like tail -f)
roxy logs --clear      # clear the log file
```

Set the daemon log level in your user configuration and restart:

```toml
[daemon]
log_level = "debug"
```

```bash
roxy restart
```

For an interactive foreground process with custom unprivileged
ports, `ROXY_LOG=debug` overrides the configured level. The
default level is `info`.

Available levels: `error`, `warn`, `info`, `debug`.

## Shell Completions

Generate tab completions for your shell with
`roxy completions <shell>`, then restart the shell.

**Fish:**

```bash
roxy completions fish > ~/.config/fish/completions/roxy.fish
```

**Zsh:**

```bash
roxy completions zsh > ~/.zfunc/_roxy
```

Make sure `~/.zfunc` is in your fpath. Add to `.zshrc`:

```bash
fpath=(~/.zfunc $fpath)
autoload -Uz compinit && compinit
```

**Bash:**

```bash
roxy completions bash \
  > ~/.local/share/bash-completion/completions/roxy
```

After setup, press `Tab` to complete commands, options,
and arguments.

## Global Options

All commands accept these global flags:

| Flag | Default | Description |
| ---- | ------- | ----------- |
| `-c`, `--config <PATH>` | Platform user config | Config file |
| `-v`, `--verbose` | off | Enable debug output |

Example using a custom config:

```bash
roxy -c "$HOME/.config/roxy-dev.toml" start
```

## Configuration

The default configuration lives at
`~/Library/Application Support/Roxy/config.toml` on macOS
and `~/.config/roxy/config.toml` on Linux.
Override it with `--config`.

### Daemon Section

```toml
[daemon]
http_port = 80
https_port = 443
dns_port = 1053
log_level = "info"
```

All three ports must be different. For an installed service,
launchd or systemd owns the privileged HTTP and HTTPS ports;
the daemon itself remains unprivileged.

### Domain Sections

Each registered domain gets its own section:

```toml
[domains.myapp-roxy]
domain = "myapp.roxy"
https_enabled = true

[[domains.myapp-roxy.routes]]
path = "/"
target = "127.0.0.1:3000"

[[domains.myapp-roxy.routes]]
path = "/api"
target = "127.0.0.1:3001"
```

Domain names must end with `.roxy` and can contain
letters, numbers, hyphens, and dots (for subdomains).
Wildcard registrations use a `*.` prefix
(see [Wildcard Subdomains](#wildcard-subdomains)).

### Paths Section

Override where Roxy stores its data:

```toml
[paths]
data_dir = "/Users/me/Library/Application Support/Roxy"
pid_file = "/Users/me/Library/Caches/Roxy/roxy.pid"
log_file = "/Users/me/Library/Logs/Roxy/roxy.log"
socket_path = "/Users/me/Library/Caches/Roxy/roxy.sock"
```

This macOS example illustrates the available fields. Linux
defaults follow the user paths described above. You only need
this section if you want different locations.

## Docker Integration

Roxy can automatically discover Docker Compose services and
register them as `.roxy` domains. Enable it in `config.toml`:

```toml
[docker]
enabled = true
```

Once enabled and the daemon is restarted, starting a compose
stack automatically registers domains like
`<service>.<project>.roxy` with HTTPS. No manual
`roxy register` needed.

See [docker.md](docker.md) for the full guide: labels
reference, container-to-Roxy communication, troubleshooting,
and more.

## Troubleshooting

### Browser Shows "Not Secure" or Certificate Warnings

**If you installed Roxy with your browser already open**, the browser won't
immediately pick up the newly trusted Root CA from the system trust store.

**Solution:** Restart your browser completely after running `sudo roxy install`.
Browsers cache the trusted certificate list at startup.

**Linux with snap browsers:** Snap-packaged browsers (Firefox, Chromium)
are sandboxed and cannot access the system trust store. See the
[Linux guide](linux.md#snap-browsers-and-certificate-trust) for a one-time
fix using `certutil`.

### A Newly Registered Domain Does Not Respond

Registration automatically reloads the running daemon. If an
external edit or watcher error prevented that reload, request
one explicitly:

```bash
roxy reload
```

### "Connection Refused" or "This site can't be reached"

Check if the daemon is running:

```bash
roxy status
```

If it's not running, start it:

```bash
roxy start
```

Verify DNS is working:

```bash
# macOS
dig myapp.roxy

# Linux
resolvectl query myapp.roxy
```

### Port Already in Use

If Roxy can't start because ports 80, 443, or 1053 are in use:

```bash
# Check what's using port 80 (HTTP)
sudo lsof -i :80

# Check what's using port 443 (HTTPS)
sudo lsof -i :443

# Check what's using port 1053 (DNS)
sudo lsof -i :1053
```

Stop the conflicting service or configure Roxy to use
different ports in your user configuration file, then rerun
`sudo roxy install` so the socket units use the new ports.

### Backend Service Not Responding

Make sure your backend service is actually running on the port you configured:

```bash
# Test if your service is listening
curl http://localhost:3000

# If that works but https://myapp.roxy doesn't, check Roxy's logs
roxy logs -f
```
