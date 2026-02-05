# Roxy

![Roxy](assets/roxy-logo-small.png)

A lightweight local dev proxy — custom domains and auto HTTPS for your localhost.

> **Early development** — Roxy is still finding her feet. Things may shift around.
> If something bites, [let me know](https://github.com/rbas/roxy/issues)!

---

## Why?

If you work on multiple projects, each with its own frontend and backend
running on different ports, you've probably dealt with the same headache:
putting nginx or traefik in front of everything on `localhost:80`, then
constantly juggling configs when switching between projects. Everything fights
over the same port, and it's a mess.

Roxy fixes this. Each project gets its own `.roxy` domain with path-based
routing to as many local services as you need. Roxy also generates trusted
SSL certificates, so you can catch issues HTTPS-related
problems locally instead of discovering them in production.

No YAML files. No containers. Just a single binary.

## Quick Start

### Install

```bash
# Build from source
cargo install --path .

# One-time setup (creates Root CA, configures DNS)
# This will trigger a system password dialog to add the Root CA
# to your macOS Keychain so that generated certificates are trusted.
sudo roxy install
```

### Register a Domain

Say you're working on a project with a frontend on port 3000 and an API on
port 3001:

```bash
roxy register myapp.roxy --route "/=3000" --route "/api=3001"
```

That's it. Start the daemon and you're good to go:

```bash
sudo roxy start
```

Now open `https://myapp.roxy` in your browser. Requests to `/` go to port
3000, requests to `/api/*` go to port 3001. HTTPS just works, no certificate
warnings.

### Switch Projects

When you move to another project, just register another domain:

```bash
roxy register other-project.roxy --route "/=8080" \
  --route "/api=8081" --route "/admin=8082"
```

No port conflicts. No config files to edit. Both `myapp.roxy` and
`other-project.roxy` work at the same time.

## Usage

```bash
# Register a domain with routes
roxy register <domain> --route "PATH=TARGET" [--route "PATH=TARGET" ...]

# Targets can be:
#   Port:        --route "/=3000"              → proxies to 127.0.0.1:3000
#   Host:Port:   --route "/=192.168.1.50:3000" → proxies to a specific host
#   Directory:   --route "/static=/var/www"     → serves static files

# Manage routes on existing domains
roxy route add myapp.roxy /webhooks 9000
roxy route remove myapp.roxy /webhooks
roxy route list myapp.roxy

# Unregister a domain
roxy unregister myapp.roxy

# List all registered domains
roxy list

# Daemon control
sudo roxy start             # Start in background
sudo roxy start --foreground # Start in foreground
sudo roxy stop
sudo roxy restart
roxy status

# Reload config without restarting
roxy reload

# View logs
roxy logs
roxy logs -f          # Follow (like tail -f)
roxy logs -n 100      # Last 100 lines
```

## How It Works

1. `roxy install` creates a local Root CA and adds it to your system trust
   store. It also configures DNS so that all `.roxy` domains resolve to
   `127.0.0.1`.

2. `roxy register` creates a trusted SSL certificate for the domain (signed
   by the Root CA) and saves the routing configuration.

3. `roxy start` launches a daemon that listens on ports 80 and 443. It
   routes incoming requests based on the `Host` header and path prefix,
   forwarding them to the right local service. WebSocket connections are fully
   supported.

For configuration details, logging options, and file locations see the
[full documentation](docs/README.md).

## Requirements

- **macOS** (Linux support planned)
- **Rust** (for building from source)
- **sudo** for install/start (needs ports 80/443 and DNS configuration)

## Roadmap

- [ ] Linux support
- [ ] Docker private network DNS — resolve `.roxy` domains
  inside containers without manual `extra_hosts`
- [ ] Wildcard subdomains (e.g., `*.myapp.roxy`)
- [ ] Auto-start on boot via launchd

## License

[MIT](LICENSE.md) — Martin Voldrich
