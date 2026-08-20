# Docker Integration

Roxy can automatically discover Docker Compose services and
register them as `.roxy` domains. When enabled, starting a
compose stack instantly makes services available at
`https://<service>.<project>.roxy`.

## Enabling Docker Integration

Add the `[docker]` section to your Roxy user configuration
(`~/Library/Application Support/Roxy/config.toml` on macOS or
`$HOME/.config/roxy/config.toml` on Linux):

```toml
[docker]
enabled = true
```

Then restart the daemon:

```bash
roxy restart
```

Roxy connects to the Docker socket and watches for container
lifecycle events. When a container starts or stops, Roxy
automatically updates its routing table.

## How Auto-Discovery Works

When a container starts, Roxy evaluates it using these rules
(in order):

1. **`roxy.enable=false`** label -- skip (explicit opt-out)
2. **`roxy.enable=true`** label -- register (explicit opt-in)
3. **Compose labels + exposed port** -- register automatically
4. **Otherwise** -- skip

For compose services, the domain is derived from the project
and service names: `<service>.<project>.roxy`. For example,
a service named `web` in project `myapp` becomes
`web.myapp.roxy`.

### Requirements for Auto-Discovery

- The container must have at least one **published port**
  (`ports:` mapping in `docker-compose.yml`)
- Roxy proxies to the **host port**, not the container port
- If a container exposes multiple ports, set `roxy.port` to
  pick one (see [Labels Reference](#labels-reference))

## Quick Start

Given this `docker-compose.yml`:

```yaml
services:
  web:
    build: .
    ports:
      - "3000:3000"
```

```bash
# Enable Docker integration in Roxy config
# (add [docker] enabled = true, then restart)

# Start your compose stack
docker compose up -d

# Roxy auto-discovers and registers web.myproject.roxy
# (project name comes from directory name by default)
open https://web.myproject.roxy
```

No `roxy register` needed -- it happens automatically.

## Labels Reference

Control Roxy behavior with container labels, either in
`docker-compose.yml` or via `docker run --label`.

| Label | Values | Description |
| ----- | ------ | ----------- |
| `roxy.enable` | `true` / `false` | Force opt-in or opt-out |
| `roxy.domain` | e.g. `app.roxy` | Override the auto-generated domain |
| `roxy.port` | e.g. `8080` | Pick which container port to proxy |
| `roxy.wildcard` | `true` | Register as wildcard (`*.domain`) |

### Examples

**Custom domain:**

```yaml
services:
  web:
    build: .
    ports:
      - "3000:3000"
    labels:
      roxy.domain: "myapp.roxy"
```

**Explicit opt-in (non-compose container):**

```yaml
services:
  standalone:
    image: nginx
    ports:
      - "8080:80"
    labels:
      roxy.enable: "true"
      roxy.domain: "nginx.roxy"
```

**Multiple ports -- pick one:**

```yaml
services:
  api:
    build: .
    ports:
      - "3000:3000"
      - "9090:9090"
    labels:
      roxy.port: "3000"
```

**Wildcard subdomains:**

```yaml
services:
  web:
    build: .
    ports:
      - "3000:3000"
    labels:
      roxy.wildcard: "true"
```

This registers `*.web.myproject.roxy`, so
`anything.web.myproject.roxy` routes to the container.

**Opt out a service:**

```yaml
services:
  db:
    image: postgres
    ports:
      - "5432:5432"
    labels:
      roxy.enable: "false"
```

## Domain Name Resolution

Roxy determines the domain in this order:

1. **`roxy.domain` label** -- used as-is (must end with `.roxy`)
2. **Compose labels** -- `<service>.<project>.roxy`

The compose project name defaults to the directory name. You
can set it explicitly with `COMPOSE_PROJECT_NAME` or the
`name:` key in `docker-compose.yml`:

```yaml
name: myapp

services:
  web:
    build: .
    ports:
      - "3000:3000"
# Domain: web.myapp.roxy
```

## Container-to-Roxy Communication

When one container needs to reach another container's
`.roxy` domain (or any `.roxy` domain served by the host),
Docker's default DNS won't resolve `.roxy` names. Use
`extra_hosts` to point the domain at the host:

```yaml
services:
  web:
    build: .
    ports:
      - "3000:3000"

  worker:
    build: .
    extra_hosts:
      - "web.myproject.roxy:host-gateway"
```

`host-gateway` resolves to the host machine's IP (typically
`host.docker.internal` on Docker Desktop). The `worker`
container can now reach `http://web.myproject.roxy` through
Roxy on the host.

Add one entry per `.roxy` domain the container needs to
access.

> **Note:** Wildcard `extra_hosts` (e.g., `*.roxy`) are not
> supported -- `/etc/hosts` does not allow wildcards. You must
> list each domain explicitly.

### Linux vs macOS

On **Linux**, Docker containers share the host network
namespace (or use a bridge with direct host access). DNS
resolution and HTTP connectivity work without `extra_hosts`
in most setups.

On **macOS**, Docker Desktop runs containers inside a Linux
VM. Containers cannot reach the host via its LAN IP, but can
reach it via `host-gateway` / `host.docker.internal`. The
`extra_hosts` approach above is required for container-to-Roxy
communication.

## Monitoring

Check which Docker containers Roxy has discovered:

```bash
roxy list
```

Docker-discovered domains show their source as "external" in
the output. They appear alongside manually registered domains
but cannot be edited with `roxy register` or `roxy route` --
they are managed entirely by the Docker watcher.

View discovery logs:

```bash
# After setting daemon.log_level = "debug" in the Roxy config
roxy restart

# Or check the log file
roxy logs -f
```

Example log output:

```text
INFO Docker integration enabled
INFO Docker domain added domain=web.myapp.roxy target=127.0.0.1:3000
INFO Docker reconciliation complete added=1 removed=0 total=1
```

## Troubleshooting

### Container Not Discovered

Check that:

1. Docker integration is enabled (`[docker] enabled = true`)
2. The container has published ports (`ports:` in compose)
3. The container is not opted out (`roxy.enable=false`)
4. The daemon is running (`roxy status`)

Set `daemon.log_level = "debug"` in the Roxy config, restart,
and follow the log to see why a container was skipped:

```bash
roxy restart
roxy logs -f
```

Look for `Docker container skipped` messages with a reason.

### "No published host port" Error

Roxy needs a host port mapping to proxy traffic. Make sure
your service has a `ports:` entry:

```yaml
services:
  web:
    build: .
    # This is required:
    ports:
      - "3000:3000"
    # EXPOSE alone is not enough
```

### Multiple Ports Without Label

If a container exposes more than one port and no `roxy.port`
label is set, Roxy skips it (ambiguous). Add the label:

```yaml
labels:
  roxy.port: "3000"
```

### Docker Socket Permission

Roxy connects to the Docker socket
(`/var/run/docker.sock` by default) as your developer account.
If you see connection errors, verify the socket exists and that
your account can access it. On Linux this commonly means adding
the account to the `docker` group (then logging in again) or
using a rootless Docker socket.
