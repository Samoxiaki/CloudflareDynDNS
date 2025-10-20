# CloudflareDynDNS
Cloudflare Dynamic DNS updater.

## Configuration Options
```env
# Cloudflare API Key
CF_TOKEN=token

# Domains to update (Comma separated)
CF_DOMAINS=domain1.com,domain2.com

# IPv4 update, optional, enabled by default
CF_IPV4_ENABLED=true

# IPv6 update, optional, disabled by default
CF_IPV6_ENABLED=false

# Proxied, optional, disabled by default
CF_PROXIED=false

# Update interval in seconds, optional, 300 (5min) by default
CF_UPDATE_INTERVAL=300

```

## Systemd Unit
```systemd
[Unit]
Description=Cloudflare Dynamic DNS Updater
After=network.target

[Service]
ExecStart=/usr/bin/cloudflaredyndns
Restart=always
Environment="CF_TOKEN=token"
Environment="CF_DOMAINS=domain1.com,domain2.com"
Environment="CF_IPV4_ENABLED=true"
Environment="CF_IPV6_ENABLED=false"
Environment="CF_PROXIED=false"
Environment="CF_UPDATE_INTERVAL=300"

[Install]
WantedBy=multi-user.target
```
