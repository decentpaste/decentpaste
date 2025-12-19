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
│   ├── api/updater.ts      # Auto-update logic
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
| `src/api/updater.ts`             | Auto-update check and install logic           |

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

## Auto-Updates (Desktop Only)

DecentPaste uses Tauri's updater plugin with GitHub Releases as the distribution backend.

### How it works:
1. App checks for updates every **60 seconds** (fetches `latest.json` from GitHub Releases)
2. If update available → orange badge on Settings nav + "Update available!" card
3. User clicks "Download & Install" → progress bar shows download
4. Download complete → app restarts and applies update

### Key files:
- `src/api/updater.ts` - Frontend update logic (check, download, install)
- `src-tauri/tauri.conf.json` - Updater config (pubkey, endpoints)
- `.github/workflows/release.yml` - CI/CD for building signed releases

### Release process:
```bash
# Bump version in tauri.conf.json and package.json
git tag v0.2.0
git push origin main --tags
# GitHub Action builds, signs, and publishes to Releases
```

### Security:
- All artifacts are signed with a private key (stored in GitHub Secrets)
- App verifies signature using embedded public key before installing
- HTTPS enforced in production

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

## Mobile Background Sync (Android & iOS)

Both Android and iOS handle background clipboard sync similarly, but with platform-specific mechanisms.

### How it works (both platforms):
1. When clipboard received in background → **silently queued** (no notification spam)
2. When app opens → clipboard is automatically copied
3. **Pairing requests** show notifications (essential - has timeout, requires user action)

### Platform Differences:

| Aspect | Android | iOS |
|--------|---------|-----|
| Background execution | Foreground Service keeps app alive indefinitely | App suspended after ~30 seconds |
| Network in background | Stays connected via wake lock | Connections drop when suspended |
| Notification | Persistent "Syncing clipboard" notification | No persistent notification |

### Android-Specific:
- **Foreground Service** (`ClipboardSyncService.kt`) starts when app launches
- Shows persistent notification: "Syncing clipboard in background"
- Keeps libp2p network connections alive via wake lock
- Permissions: `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_DATA_SYNC`, `POST_NOTIFICATIONS`, `WAKE_LOCK`

### iOS-Specific:
- No foreground service (iOS doesn't allow Android-style persistent background execution)
- Network connections drop when app is suspended
- When app resumes, automatically reconnects to peers
- Background clipboard queuing works if clipboard arrives before app suspension

### Key files:
- `src-tauri/src/lib.rs` - Rust-side background detection and pending clipboard handling
- `src-tauri/src/state.rs` - `PendingClipboard` struct and `is_foreground` tracking
- Android: `src-tauri/gen/android/app/src/main/java/com/decentpaste/application/ClipboardSyncService.kt`
- iOS: Uses standard Tauri lifecycle events (no native Swift code needed)

### Clipboard Background Limitation (both platforms):
Mobile OSes block clipboard access in background. When clipboard arrives while app is backgrounded:
1. Content is silently queued in `AppState.pending_clipboard` (no notification)
2. When app becomes visible, frontend calls `processPendingClipboard()` command
3. Rust copies the pending content to clipboard and returns it
4. Frontend shows toast: "Clipboard synced from {device}"

**Pairing requests** are the exception - they DO show notifications because they require immediate user action and have a 5-minute timeout.

### Commands:
- `process_pending_clipboard` - Called by frontend when app becomes visible
  - Returns: `{ content: string, from_device: string }` or `null`
  - Copies pending clipboard and clears the queue

### Events:
- `clipboard-synced-from-background` - Emitted when pending clipboard is copied on resume
  - Payload: `{ content: string, fromDevice: string }`

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **In-memory history** - Not persisted to disk
- **Mobile clipboard (outgoing)** - Auto-monitoring disabled on Android/iOS; use "Share Clipboard" button
- **Mobile clipboard (incoming)** - Can receive in background (Android: via foreground service; iOS: limited ~30s window), actual copy happens on resume due to OS restrictions
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
