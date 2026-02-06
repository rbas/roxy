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

| Command                            | Description                |
| ---------------------------------- | -------------------------- |
| `sudo roxy install`                | Initial setup              |
| `sudo roxy uninstall [--force]`    | Full cleanup               |
| `sudo roxy register <domain> ...`  | Register domain            |
| `roxy unregister <domain>`         | Remove domain              |
| `roxy list`                        | Show all domains           |
| `sudo roxy route add ...`          | Add route to domain        |
| `roxy route remove ...`            | Remove route from domain   |
| `roxy route list <domain>`         | List routes for domain     |
| `sudo roxy start [--foreground]`   | Start daemon               |
| `sudo roxy stop`                   | Stop daemon                |
| `sudo roxy restart`                | Restart daemon             |
| `sudo roxy reload`                 | Reload configuration       |
| `roxy status`                      | Show daemon status         |
| `roxy logs [-n N] [-f]`            | View or follow daemon logs |

**Note:** Commands that modify system configuration (CA certs, DNS) or control the daemon (runs on ports 80/443) require `sudo`.

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

## Troubleshooting

### Browser Shows "Not Secure" or Certificate Warnings

**If you installed Roxy with your browser already open**, the browser won't
immediately pick up the newly trusted Root CA from the system keychain.

**Solution:** Restart your browser completely after running `sudo roxy install`.
Browsers cache the trusted certificate list at startup.

### Certificates Show Wrong Domain Name

If accessing `myapp.roxy` shows a certificate for a different domain, the daemon
needs to be restarted to pick up newly registered domains.

**Solution:** Run `sudo roxy restart` after registering new domains.

### "Connection Refused" or "This site can't be reached"

Check if the daemon is running:

```bash
roxy status
```

If it's not running, start it:

```bash
sudo roxy start
```

Verify DNS is working:

```bash
dig myapp.roxy
# Should show: myapp.roxy. 0 IN A 127.0.0.1
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

Stop the conflicting service or configure Roxy to use different ports
in `~/.roxy/config.toml`.

### Backend Service Not Responding

Make sure your backend service is actually running on the port you configured:

```bash
# Test if your service is listening
curl http://localhost:3000

# If that works but https://myapp.roxy doesn't, check Roxy's logs
roxy logs -f
```
