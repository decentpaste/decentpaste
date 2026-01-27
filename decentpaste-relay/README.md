# DecentPaste Relay Server

A libp2p relay server that enables DecentPaste clients behind NAT to connect to each other.

## Security Properties

- **E2E Encryption**: All clipboard content is AES-256-GCM encrypted before reaching the relay
- **No Content Access**: The relay sees only encrypted bytes and metadata (PeerIds, connection timing)
- **Privacy**: PeerIds are pseudonymous (derived from Ed25519 public keys)

## What the Relay CAN See

- PeerIds (pseudonymous identifiers)
- IP addresses of connected clients
- Connection times and durations
- Traffic volume (but not content)

## What the Relay CANNOT See

- Clipboard content (encrypted with per-peer shared secrets)
- Decryption keys (never transmitted, derived via ECDH)
- Any message content

## Running

**Important**: You must specify the `--external-ip` flag with your server's public IP address. Without this, relay reservations will fail because clients won't know how to reach you.

```bash
# Production (replace with your actual public IP)
cargo run -- --port 4001 --health-port 8080 --external-ip xx.xx.xx.xx

# Local testing (won't work for real NAT traversal)
cargo run -- --port 4001 --health-port 8080
```

## Docker

```bash
# Build
docker build -t decentpaste-relay .

# Run (replace YOUR_PUBLIC_IP with your server's actual public IP)
docker run -d \
  --name decentpaste-relay \
  -p 4001:4001 \
  -p 8080:8080 \
  decentpaste-relay --external-ip YOUR_PUBLIC_IP
```

## Health Check

```bash
# Check health
curl http://localhost:8080/health

# Get relay info (includes Peer ID)
curl http://localhost:8080/info
```

## Configuration

| Flag | Default | Description |
|------|---------|-------------|
| `--port` | 4001 | libp2p listening port |
| `--health-port` | 8080 | HTTP health check port |
| `--external-ip` | (required) | Public IP address of the relay server |
| `--max-reservations` | 1000 | Maximum concurrent relay reservations |
| `--max-circuit-duration-secs` | 1800 | Maximum circuit duration (30 min) |

## Environment Variables

- `RUST_LOG`: Logging level (e.g., `info`, `debug`, `decentpaste_relay=debug`)

## Deployment Notes

1. **Public IP Required**: The relay must have a public IP address
2. **Firewall**: Open port 4001 (TCP) for libp2p
3. **DNS**: Set up DNS record (e.g., `relay-us.decentpaste.com`)
4. **Monitoring**: Use the `/health` endpoint for health checks
