# Roxy

Roxy is a local development proxy for macOS that gives
your projects custom `.roxy` domains with automatic HTTPS.
It ships as a single binary with no external dependencies.
Register a domain, point it at a port or directory, and
open `https://myapp.roxy` in your browser.

## Quick Start

```bash
# Initial setup — installs Root CA, configures DNS,
# trusts the certificate in macOS Keychain
sudo roxy install

# Register a domain that proxies to localhost:3000
roxy register myapp.roxy --route "/=3000"

# Start the daemon (requires sudo for ports 80/443)
sudo roxy start

# Open in browser
open https://myapp.roxy
```

## Commands Reference

| Command                      | Description                |
| ---------------------------- | -------------------------- |
| `roxy install`               | Initial setup              |
| `roxy uninstall [--force]`   | Full cleanup               |
| `roxy register <domain> ...` | Register domain            |
| `roxy unregister <domain>`   | Remove domain              |
| `roxy list`                  | Show all domains           |
| `roxy route add ...`         | Add route to domain        |
| `roxy route remove ...`      | Remove route from domain   |
| `roxy route list <domain>`   | List routes for domain     |
| `roxy start [--foreground]`  | Start daemon               |
| `roxy stop`                  | Stop daemon                |
| `roxy restart`               | Restart daemon             |
| `roxy reload`                | Reload configuration       |
| `roxy status`                | Show daemon status         |
| `roxy logs [-n N] [-f]`      | View or follow daemon logs |

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

## Files and Directories

```text
~/.roxy/
├── config.toml          # Main configuration
├── roxy.pid             # PID file (when daemon runs)
├── certs/
│   ├── ca.key           # Root CA private key
│   ├── ca.crt           # Root CA certificate
│   ├── <domain>.key     # Per-domain private key
│   └── <domain>.crt     # Per-domain certificate
└── logs/
    └── roxy.log         # Daemon log file
```

macOS DNS resolver file (created by `roxy install`):

```text
/etc/resolver/roxy
```

This tells macOS to resolve all `*.roxy` domains through
the local DNS server.

## Daemon: Foreground vs Background

**Background** (default) — forks to the background,
writes a PID file, logs to `~/.roxy/logs/roxy.log`:

```bash
sudo roxy start
```

**Foreground** — stays in the terminal, logs to stdout,
stop with Ctrl+C. Useful for debugging:

```bash
sudo roxy start --foreground
```

## Logging and Verbosity

View logs:

```bash
roxy logs              # last 50 lines
roxy logs -n 100       # last 100 lines
roxy logs -f           # follow (like tail -f)
roxy logs --clear      # clear the log file
```

Change the log level (highest priority first):

1. **Environment variable** —
   `ROXY_LOG=debug sudo roxy start`
2. **CLI flag** — `sudo roxy start --verbose`
   (sets debug level)
3. **Config file** — edit `~/.roxy/config.toml`:

   ```toml
   [daemon]
   log_level = "debug"
   ```

4. **Default** — `info`

Available levels: `error`, `warn`, `info`, `debug`.

## Configuration

The configuration lives in `~/.roxy/config.toml`.

### Daemon Section

```toml
[daemon]
http_port = 80
https_port = 443
dns_port = 1053
log_level = "info"
```

All three ports must be different. The daemon needs
`sudo` to bind to ports below 1024.

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

## Using Roxy with Docker

Roxy runs on the host, so containers need to know how
to reach `.roxy` domains. Add the domain as an extra host
pointing to the host gateway in your `docker-compose.yml`:

```yaml
services:
  myservice:
    image: myimage
    extra_hosts:
      - "myservice.roxy:host-gateway"
```

`host-gateway` resolves to the host machine's IP
(typically `host.docker.internal` on Docker Desktop).
The container can now reach `http://myservice.roxy` or
`https://myservice.roxy` through Roxy on the host.

Add one entry per `.roxy` domain the container needs
to access.
