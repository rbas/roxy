# Roxy on Linux

This guide covers Linux-specific setup details and
troubleshooting. For general usage, see the
[main documentation](README.md).

## Supported Distributions

Roxy is tested on **Ubuntu 22.04+** and **Debian 12+**.
It should work on any distribution that uses:

- **systemd-resolved** for DNS
- **update-ca-certificates** for the system trust store

Other distributions (Fedora, Arch, etc.) may work but
are not yet officially supported.

## How It Works on Linux

### DNS Resolution

On macOS, Roxy uses `/etc/resolver/roxy` to route `.roxy`
DNS queries. On Linux, it uses **systemd-resolved** with
a drop-in configuration file:

```text
/etc/systemd/resolved.conf.d/roxy.conf
```

This file tells systemd-resolved to forward all `.roxy`
queries to Roxy's built-in DNS server on port 1053.
Other DNS queries are unaffected.

The file is created automatically by `sudo roxy install`
and removed by `sudo roxy uninstall`.

### Certificate Trust

On macOS, the Root CA is added to the system Keychain.
On Linux, Roxy copies the Root CA to the system
certificate store and runs `update-ca-certificates`:

```text
/usr/local/share/ca-certificates/roxy-ca.crt
```

This makes the CA trusted by `curl`, `wget`, `git`,
Electron apps, and non-sandboxed browsers.

## Snap Browsers and Certificate Trust

**This is the most common issue on Linux.**

Snap-packaged browsers (Firefox, Chromium) run in a
sandbox and **cannot access the system's certificate
trust store**. Even with proper system CA installation,
these browsers will show certificate warnings for
`.roxy` domains.

This does **not** affect:

- `curl`, `wget`, `git`, and other CLI tools
- Non-snap browsers (installed via apt/deb)
- Electron apps (VS Code, Slack, etc.)

### Fix: Import the CA with certutil

This is a **one-time** step per browser. Once the Root CA
is imported, all `.roxy` domains (including newly
registered ones) are automatically trusted.

**1. Install certutil:**

```bash
sudo apt install libnss3-tools
```

**2. Import the Roxy CA into your browser:**

**Snap Firefox:**

```bash
certutil -A -n "Roxy Local Development CA" -t "CT,C,C" \
  -i /etc/roxy/ca.crt \
  -d sql:$(find ~/snap/firefox/common/.mozilla/firefox \
    -name '*.default*' -type d | head -1)/
```

**Snap Chromium:**

```bash
certutil -A -n "Roxy Local Development CA" -t "CT,C,C" \
  -i /etc/roxy/ca.crt \
  -d sql:$(find ~/snap/chromium -name 'nssdb' \
    -type d | head -1)/
```

No browser restart is needed after import.

### Why Does This Happen?

Snap applications are sandboxed using AppArmor and
seccomp. They cannot load the host system's p11-kit
trust modules, which is how most Linux applications
discover trusted CAs. The `certutil` command writes
directly into the browser's own NSS certificate
database, bypassing the sandbox limitation.

### Removing the CA

To remove the Roxy CA from a snap browser:

**Firefox:**

```bash
certutil -D -n "Roxy Local Development CA" \
  -d sql:$(find ~/snap/firefox/common/.mozilla/firefox \
    -name '*.default*' -type d | head -1)/
```

**Chromium:**

```bash
certutil -D -n "Roxy Local Development CA" \
  -d sql:$(find ~/snap/chromium -name 'nssdb' \
    -type d | head -1)/
```

## Troubleshooting

### DNS Not Resolving

If `curl https://myapp.roxy` fails with "Could not
resolve host":

**1. Check if systemd-resolved has the config:**

```bash
resolvectl status
```

Look for `DNS Servers: 127.0.0.1:1053` and
`DNS Domain: ~roxy` in the global section.

**2. Check if the Roxy DNS server responds:**

```bash
dig @127.0.0.1 -p 1053 myapp.roxy
```

If this works but `resolvectl query` doesn't, the
drop-in config may need a restart:

```bash
sudo systemctl restart systemd-resolved
```

**3. Verify the drop-in file exists:**

```bash
cat /etc/systemd/resolved.conf.d/roxy.conf
```

If missing, re-run `sudo roxy install`.

### Port Already in Use

On Linux, check which process is using a port with `ss`:

```bash
sudo ss -tlnp | grep ':80\b'
sudo ss -tlnp | grep ':443\b'
sudo ss -tlnp | grep ':1053\b'
```

Common culprits: Apache (`apache2`), nginx, or another
Roxy instance. Stop the conflicting service or change
Roxy's ports in `/etc/roxy/config.toml`.

### Auto-Start with systemd

Create a service file to start Roxy at boot:

```bash
sudo tee /etc/systemd/system/roxy.service > /dev/null <<'EOF'
[Unit]
Description=Roxy local development proxy
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/roxy start --foreground
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable --now roxy
```

Check status:

```bash
sudo systemctl status roxy
```
