# Roxy

![Roxy](assets/roxy-logo-small.png)

![License](https://img.shields.io/badge/license-MIT-blue.svg)

**Stop juggling localhost ports.** Get real domains and trusted HTTPS for every
local project — **with zero configuration files**.

Full visibility into all traffic, including WebSockets. No YAML. No containers.
Just a single binary.

**The Problem:**

- Which localhost port was my frontend again? 3000? 8080?
- Browser screaming about invalid certificates every time you test HTTPS
- Can't test OAuth callbacks locally (they require HTTPS)
- Can't share your local work with your phone or teammate's laptop
- Running 5 microservices = memorizing 5 ports

**Without Roxy:**

- `http://localhost:3000`   ← frontend? backend?
- `http://localhost:3001`   ← which service?
- `http://localhost:8080`   ← another project

**With Roxy:**

- `https://myapp.roxy`      ← frontend (port 3000)
- `https://myapp.roxy/api`  ← backend (port 3001)
- `https://other.roxy`      ← different project (port 8080)

One domain, multiple services. Path-based routing with trusted HTTPS.

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

## See It In Action

![DEMO](assets/roxy-demo.gif)

## Perfect For

- 🚀 **Full-stack developers** running frontend + backend + database locally
- 📱 **Mobile app developers** testing APIs that require HTTPS
- 🔗 **OAuth/webhook development** that requires real HTTPS callbacks
- 👥 **Teams** who need to share work across devices on the same network
- 🔧 **Microservices developers** juggling multiple local services
- 🎨 **Anyone tired of memorizing port numbers**

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

## Real-World Example

Testing Stripe webhooks locally? They require HTTPS. Here's how:

```bash
# Register your e-commerce app
roxy register shopify-clone.roxy \
  --route "/=3000" \
  --route "/api=3001"

# Now point Stripe webhooks to: https://shopify-clone.roxy/api/webhooks
# No ngrok. No tunnels. Just works.
```

Works with **any stack**: Next.js, Rails, Django, Express, Flask, Go, Rust,
PHP... anything that speaks HTTP on any port.

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
sudo roxy reload

# View logs
roxy logs
roxy logs -f          # Follow (like tail -f)
roxy logs -n 100      # Last 100 lines
```

## How It Works

**Three steps, then forget about it:**

1. **`roxy install`** → Creates trusted Root CA + configures DNS for `.roxy` domains
2. **`roxy register`** → Generates SSL cert + saves routing config
3. **`roxy start`** → Runs proxy on `:80`/`:443`, routes traffic to your services

Your browser trusts the certificates (no warnings). WebSockets are fully supported.

**Clean and contained:** Everything lives in `~/.roxy/` (config, certs, logs) and
`/etc/resolver/roxy` (DNS). Run `roxy uninstall` to remove everything cleanly.

For configuration details, logging options, and file locations see the
[full documentation](docs/README.md).

## How Is This Different?

**Unlike nginx/traefik** — Zero configuration files. No YAML, no `sites-enabled`,
no nginx.conf, no config syntax to learn or get wrong. Just one command to register
a domain.

**Unlike Laravel Valet** — No dnsmasq, no nginx, no PHP runtime. Roxy has its
own built-in DNS server and reverse proxy in a single binary. Works with any
stack on any port, not just PHP.

**Unlike `/etc/hosts` hacks** — Real HTTPS with trusted certificates (not
self-signed warnings), path-based routing, WebSocket support, and traffic logging.

**Unlike ngrok/tunnels** — Everything stays local. No third-party services, no
bandwidth limits, no latency. Your traffic never leaves your machine.

## Requirements

- **macOS** (Linux support planned)
- **Rust** (for building from source)
- **sudo** for install/start (needs ports 80/443 and DNS configuration)

## What's Next

Roxy is ready for daily development use on macOS. Future plans:

- [ ] **Linux support** — extend to Linux development environments
- [ ] **Docker network DNS** — resolve `.roxy` domains inside containers without `extra_hosts`
- [ ] **Wildcard subdomains** — support `*.myapp.roxy` patterns
- [ ] **Auto-start on boot** — launch daemon via launchd automatically
- [ ] **Config hot reload** — pick up changes without restarting (already works with `roxy reload`!)

Have a feature idea? [Open an issue](https://github.com/rbas/roxy/issues) and let's discuss!

---

## Get Started Now

```bash
# Build from source
cargo install --path .

# One-time setup
sudo roxy install

# Register your first project
roxy register myapp.roxy --route "/=3000"

# Start the proxy
sudo roxy start
```

Visit `https://myapp.roxy` — no warnings, no config files, just works.

Questions? Check the [full documentation](docs/README.md) or [open an issue](https://github.com/rbas/roxy/issues).

**Find Roxy useful?** ⭐ Star the repo to help others discover it!

## Status

Roxy is in early development — things may shift around. If something bites,
[let me know](https://github.com/rbas/roxy/issues)!

## License

[MIT](LICENSE.md) — Martin Voldrich

---

Having issues? Check the [troubleshooting guide](docs/README.md#troubleshooting).
