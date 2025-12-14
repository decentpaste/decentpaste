# DecentPaste - AI Agent Quick Reference

## What is this project?

DecentPaste is a cross-platform clipboard sharing app (like Apple's Universal Clipboard) that works on all platforms. It
uses:

- **Tauri v2** for the desktop/mobile app
- **libp2p** for decentralized P2P networking
- **mDNS** for local network device discovery
- **X25519 ECDH** for secure key exchange during pairing
- **AES-256-GCM** for end-to-end encryption

## Quick Start

```bash
cd decentpaste-app
yarn install
yarn tauri dev
```

## Project Structure

```
decentpaste-app/
├── src/                    # Frontend (TypeScript + Tailwind v4)
│   ├── app.ts              # Main UI (Dashboard, Peers, History, Settings)
│   ├── api/commands.ts     # Tauri command wrappers
│   ├── api/events.ts       # Event listeners
│   └── state/store.ts      # Reactive state
└── src-tauri/src/          # Backend (Rust)
    ├── lib.rs              # App initialization
    ├── commands.rs         # Tauri command handlers (19 commands)
    ├── network/            # libp2p networking
    │   ├── behaviour.rs    # mDNS + gossipsub + request-response
    │   └── swarm.rs        # Network manager
    ├── clipboard/          # Clipboard handling
    │   ├── monitor.rs      # Polls clipboard every 500ms
    │   └── sync.rs         # Deduplication logic
    ├── security/           # Encryption & pairing
    │   ├── crypto.rs       # AES-256-GCM encryption
    │   ├── identity.rs     # X25519 keypair & ECDH key derivation
    │   └── pairing.rs      # 6-digit PIN pairing
    └── storage/            # Settings & peer persistence
```

## Key Files to Know

| File                             | Purpose                                       |
|----------------------------------|-----------------------------------------------|
| `src-tauri/src/lib.rs`           | App startup, spawns network & clipboard tasks |
| `src-tauri/src/commands.rs`      | All Tauri commands                            |
| `src-tauri/src/network/swarm.rs` | libp2p network manager                        |
| `src/app.ts`                     | All frontend UI in one file                   |
| `src/api/types.ts`               | TypeScript interfaces                         |

## How Clipboard Sync Works

1. `ClipboardMonitor` polls every 500ms, hashes content
2. If content changed & is local → encrypt separately for **each paired peer** using their specific shared secret
3. Broadcast via gossipsub (one message per peer)
4. Receiving peer decrypts with their shared secret, updates clipboard
5. Hash tracked to prevent echo loops

## How Pairing Works (X25519 ECDH Key Exchange)

1. Device A sends `PairingRequest` with **X25519 public key**
2. Device B stores A's public key, generates 6-digit PIN
3. Device B sends `PairingChallenge` with PIN and **own public key**
4. Device A stores B's public key, displays PIN
5. User confirms PIN matches on both devices
6. Both devices independently derive **same shared secret** via ECDH:
   - `shared_secret = ECDH(my_private_key, peer_public_key)`
7. Stored in `~/.local/share/com.decentpaste.app/peers.json`

**Security**: No secret is transmitted - both sides derive it from the exchanged public keys.

## Persistent Identity

- **libp2p keypair** is persisted in `~/.local/share/com.decentpaste.app/libp2p_keypair.bin`
- This ensures the PeerId stays consistent across app restarts
- Without this, paired devices would appear as "new" after restart

## Connection Resilience

- **Automatic retry** - Failed connections retry up to 3 times with 2-second delays
- **Explicit gossipsub peers** - Connected peers are added to gossipsub mesh immediately for faster message delivery
- **Mobile background handling** - When app returns from background, it automatically reconnects to all discovered peers
  via visibility change listener

## Device Name Broadcasting

Device names are propagated to peers through multiple mechanisms:

1. **Identify protocol** - On startup, device name is included in libp2p identify's `agent_version` field
   (format: `decentpaste/<version>/<device_name>`)
2. **DeviceAnnounce message** - When name changes in settings, a `DeviceAnnounce` gossipsub message is broadcast
3. **On connection** - When a new peer connects, we automatically announce our device name to ensure they have the
   current name (handles the case where they were offline when we changed it)

## Unpair → Rediscovery

When you unpair a device:
1. The peer is removed from `paired_peers`
2. A `RefreshPeer` command is sent to NetworkManager
3. If the peer is still on the network, it's re-emitted as a discovered peer
4. No app restart required to pair again

## Common Tasks

### Add a new Tauri command

1. Add to `src-tauri/src/commands.rs`:

```rust
#[tauri::command]
pub async fn my_command(state: State<'_, AppState>, arg: String) -> Result<String> {
    // implementation
}
```

2. Register in `src-tauri/src/lib.rs`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    commands::my_command,
])
```

3. Add TypeScript wrapper in `src/api/commands.ts`:

```typescript
export async function myCommand(arg: string): Promise<string> {
    return invoke('my_command', {arg});
}
```

### Add a new event

1. Emit from Rust:

```rust
app_handle.emit("my-event", payload) ?;
```

2. Listen in `src/api/events.ts`:

```typescript
listen<MyPayload>('my-event', (e) => {
    // handle
});
```

### Modify the UI

All UI is in `src/app.ts`. Key methods:

- `renderDashboard()` - Home view
- `renderPeersView()` - Peers list
- `renderHistoryView()` - Clipboard history
- `renderSettingsView()` - Settings page
- `renderPairingModalContent()` - Pairing dialog

## Design Decisions

1. **Why libp2p?** Decentralized, no central server, built-in encryption
2. **Why polling clipboard?** Tauri clipboard plugin doesn't support native change events
3. **Why gossipsub?** Efficiently broadcasts to multiple peers
4. **Why request-response for pairing?** Needs reliable 1:1 communication
5. **Why shared secret per pair?** Each device pair has unique encryption key
6. **Why X25519 ECDH?** Industry-standard key exchange - shared secret derived, never transmitted

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **In-memory history** - Not persisted to disk
- **Mobile clipboard** - Auto-monitoring disabled on Android/iOS; use "Share Clipboard" button on Dashboard
- **Plaintext storage** - Shared secrets stored in JSON without OS keychain integration

## Testing

```bash
# Run two instances on same machine:
# Terminal 1
yarn tauri dev

# Terminal 2 (different port)
TAURI_DEV_PORT=1421 yarn tauri dev
```

## See Also

- `ARCHITECTURE.md` - Detailed architecture documentation
- `src-tauri/Cargo.toml` - Rust dependencies
- `package.json` - Frontend dependencies
