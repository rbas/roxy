# Roxy

![Roxy](assets/roxy-logo-small.png)

**Stop juggling localhost ports.** Get real domains and trusted HTTPS for every
local project — with full visibility into all traffic, including WebSockets.

No YAML files. No containers. Just a single binary.

```
# Without Roxy                          # With Roxy
http://localhost:3000   ← which app?    https://myapp.roxy        ← obvious
http://localhost:3001   ← is this the   https://myapp.roxy/api    ← frontend + API
http://localhost:8080   ← port conflict https://other.roxy        ← no conflicts
```

---

## Quick Start

```bash
# Build from source
cargo install --path .

# One-time setup (creates Root CA, configures DNS)
sudo roxy install

# Register a project with a frontend (port 3000) and API (port 3001)
roxy register myapp.roxy --route "/=3000" --route "/api=3001"

# Start the proxy
sudo roxy start
```

Open `https://myapp.roxy` in your browser. That's it. Trusted HTTPS, no
certificate warnings, path-based routing — all working.

### Multiple Projects, Zero Conflicts

```bash
roxy register other-project.roxy --route "/=8080" \
  --route "/api=8081" --route "/admin=8082"
```

Both `myapp.roxy` and `other-project.roxy` work at the same time. No port
fights. No config files to edit.

## Features

- **Custom `.roxy` domains** — each project gets its own domain
- **Trusted HTTPS** — auto-generated certificates your browser actually trusts
- **Path-based routing** — map different paths to different local services
- **Traffic visibility** — see every HTTP request and WebSocket connection
  flowing through Roxy with `roxy logs -f`
- **WebSocket support** — connections are proxied and tracked, with connection
  lifecycle logging
- **Static file serving** — serve a directory directly without a local server
- **LAN access** — services accessible from other devices on your network
- **Built-in DNS server** — no dnsmasq or external DNS tools needed

## See Your Traffic

Roxy logs all traffic flowing through it — HTTP requests, WebSocket connections,
and DNS queries. No more guessing what hit your backend.

```bash
# Follow traffic in real-time
roxy logs -f
```
```
INFO Request completed method=GET host=myapp.roxy path=/ status=200
INFO Request completed method=POST host=myapp.roxy path=/api/users status=201
INFO WebSocket connection established target=127.0.0.1:3000
INFO WebSocket connection closed target=127.0.0.1:3000 duration_ms=45230
INFO DNS query domain=myapp.roxy qtype=A response=127.0.0.1
```

Turn on verbose mode for full routing details:

```bash
sudo roxy start --verbose
```
```
DEBUG Routing request method=GET host=myapp.roxy path=/api/users route=/api
DEBUG Proxying HTTP request target=127.0.0.1:3001
DEBUG Proxy response target=127.0.0.1:3001 status=200
```

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
sudo roxy start              # Start in background
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

1. **`roxy install`** creates a local Root CA and adds it to your macOS
   Keychain. It also configures DNS so all `.roxy` domains resolve to
   `127.0.0.1`. Your browser trusts certificates signed by this CA — no
   warnings, no exceptions to click through.

2. **`roxy register`** generates a trusted SSL certificate for the domain and
   saves the routing configuration. Certificates are signed by the Root CA, so
   they're trusted immediately.

3. **`roxy start`** launches a daemon on ports 80 and 443. It routes requests
   based on the `Host` header and path prefix, forwarding them to the right
   local service. WebSocket connections are fully supported and tracked.

Roxy only touches `~/.roxy/` (config, certs, logs) and `/etc/resolver/roxy`
(DNS). Run `roxy uninstall` to remove everything cleanly.

For configuration details, logging options, and file locations see the
[full documentation](docs/README.md).

## How Is This Different?

**Unlike nginx/traefik** — there's nothing to configure. No YAML, no
`sites-enabled`, no config syntax to get wrong.

**Unlike Laravel Valet** — no dnsmasq, no nginx, no PHP runtime. Roxy has its
own built-in DNS server and reverse proxy in a single binary. Works with any
stack on any port.

**Unlike `/etc/hosts` hacks** — you get real HTTPS with trusted certificates,
path-based routing, and WebSocket support.

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
- [ ] Hot reload — pick up config changes without restarting the daemon

## Status

Roxy is in early development — things may shift around. If something bites,
[let me know](https://github.com/rbas/roxy/issues)!

## License

[MIT](LICENSE.md) — Martin Voldrich

---

Having issues? Check the [troubleshooting guide](docs/README.md#troubleshooting).
