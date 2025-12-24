# DecentPaste - AI Agent Quick Reference

## What is this project?

DecentPaste is a cross-platform clipboard sharing app (like Apple's Universal Clipboard) that works on all platforms. It
uses:

- **Tauri v2** for the desktop/mobile app
- **libp2p** for decentralized P2P networking
- **mDNS** for local network device discovery
- **X25519 ECDH** for secure key exchange during pairing
- **AES-256-GCM** for end-to-end encryption
- **IOTA Stronghold** for encrypted local storage (PIN-protected vault)

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
│   ├── app.ts              # Main UI (Dashboard, Peers, Settings)
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
    ├── vault/              # Encrypted storage (Stronghold)
    │   ├── auth.rs         # VaultStatus & AuthMethod types
    │   ├── manager.rs      # VaultManager (create, open, lock, flush)
    │   └── salt.rs         # Installation-specific salt
    └── storage/            # Settings & peer persistence
```

## Key Files to Know

| File                             | Purpose                                         |
|----------------------------------|-------------------------------------------------|
| `src-tauri/src/lib.rs`           | App startup, spawns network & clipboard tasks   |
| `src-tauri/src/commands.rs`      | All Tauri commands                              |
| `src-tauri/src/state.rs`         | AppState + flush helper methods                 |
| `src-tauri/src/vault/manager.rs` | VaultManager - encrypted storage lifecycle      |
| `src-tauri/src/network/swarm.rs` | libp2p network manager                          |
| `src/app.ts`                     | All frontend UI (incl. onboarding, lock screen) |
| `src/api/types.ts`               | TypeScript interfaces                           |
| `src/api/updater.ts`             | Auto-update check and install logic             |

## How Clipboard Sync Works

1. `ClipboardMonitor` polls every 500ms, hashes content
2. If `auto_sync_enabled` is false, skip broadcast (sync paused)
3. If content changed & is local → encrypt separately for **each paired peer** using their specific shared secret
4. Broadcast via gossipsub (one message per peer)
5. Receiving peer decrypts with their shared secret, updates clipboard
6. Hash tracked to prevent echo loops

## How Pairing Works (X25519 ECDH Key Exchange)

1. Device A sends `PairingRequest` with **X25519 public key**
2. Device B stores A's public key, generates 6-digit PIN
3. Device B sends `PairingChallenge` with PIN and **own public key**
4. Device A stores B's public key, displays PIN
5. User confirms PIN matches on both devices
6. Both devices independently derive **same shared secret** via ECDH:
   - `shared_secret = ECDH(my_private_key, peer_public_key)`
7. Stored encrypted in `vault.hold` (Stronghold)

**Security**: No secret is transmitted - both sides derive it from the exchanged public keys.

## Vault & Authentication

All sensitive data is stored in an encrypted IOTA Stronghold vault, protected by a user PIN.

### How it works

```
User PIN (4-8 digits)
       │
       ▼
┌─────────────────────────┐
│    Argon2id KDF         │ ← salt.bin (16 bytes, unique per install)
│ Memory: 64MB, Time: 3   │
└─────────────────────────┘
       │
       ▼
   256-bit Key
       │
       ▼
┌─────────────────────────┐
│   vault.hold            │  ← IOTA Stronghold encrypted file
│  ┌───────────────────┐  │
│  │ • paired_peers     │  │
│  │ • clipboard_history│  │
│  │ • device_identity  │  │
│  │ • libp2p_keypair   │  │
│  └───────────────────┘  │
└─────────────────────────┘
```

### Vault States (VaultStatus)

| State      | UI Shown          | Next Action      |
|------------|-------------------|------------------|
| `NotSetup` | Onboarding wizard | User creates PIN |
| `Locked`   | Lock screen       | User enters PIN  |
| `Unlocked` | Main app          | Normal usage     |

### Key Vault Commands

| Command            | Purpose                            |
|--------------------|------------------------------------|
| `get_vault_status` | Check current vault state          |
| `setup_vault`      | Create new vault during onboarding |
| `unlock_vault`     | Open vault with PIN                |
| `lock_vault`       | Flush data and lock                |
| `reset_vault`      | Destroy vault (factory reset)      |
| `flush_vault`      | Force save to disk                 |

### Data Storage Locations

| Data                   | Location        | Format         |
|------------------------|-----------------|----------------|
| Paired peers + secrets | `vault.hold`    | Encrypted      |
| Clipboard history      | `vault.hold`    | Encrypted      |
| Device identity + keys | `vault.hold`    | Encrypted      |
| libp2p keypair         | `vault.hold`    | Encrypted      |
| Salt for Argon2        | `salt.bin`      | Raw bytes      |
| App settings           | `settings.json` | Plaintext JSON |

### Flush-on-Write Pattern

Data is persisted **immediately** after every mutation, ensuring no data loss on unexpected termination:

| Mutation              | Method Called                     |
|-----------------------|-----------------------------------|
| Peer paired/unpaired  | `state.flush_paired_peers()`      |
| Peer name updated     | `state.flush_paired_peers()`      |
| Device name changed   | `state.flush_device_identity()`   |
| Clipboard entry added | `state.flush_clipboard_history()` |
| History cleared       | `state.flush_clipboard_history()` |

**Safety net flushes** (redundant but defensive):
- App backgrounded (mobile) → `flush_all_to_vault()`
- App exit (all platforms) → `flush_all_to_vault()`
- Manual lock → `flush_all_to_vault()`

## Persistent Identity

- **libp2p keypair** is stored in the encrypted `vault.hold` file
- This ensures the PeerId stays consistent across app restarts
- Without this, paired devices would appear as "new" after restart
- The keypair is only accessible when the vault is unlocked

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

**Authentication views:**
- `renderOnboarding()` - First-time setup (device name → PIN)
- `renderLockScreen()` - PIN entry for returning users
- `renderResetConfirmation()` - "Forgot PIN?" reset flow

**Main app views:**
- `renderDashboard()` - Home view
- `renderPeersView()` - Peers list
- `renderHistoryView()` - Clipboard history
- `renderSettingsView()` - Settings page + lock button
- `renderPairingModalContent()` - Pairing dialog

## Design Decisions

1. **Why libp2p?** Decentralized, no central server, built-in encryption
2. **Why polling clipboard?** Tauri clipboard plugin doesn't support native change events
3. **Why gossipsub?** Efficiently broadcasts to multiple peers
4. **Why request-response for pairing?** Needs reliable 1:1 communication
5. **Why shared secret per pair?** Each device pair has unique encryption key
6. **Why X25519 ECDH?** Industry-standard key exchange - shared secret derived, never transmitted
7. **Why Stronghold over OS keychain?** Cross-platform consistency, stores complex data, offline access
8. **Why Argon2id?** Memory-hard KDF resists GPU/ASIC brute-force attacks on PIN

## Mobile Background Sync (Android & iOS)

Both Android and iOS handle background clipboard sync identically.

### How it works (both platforms):
1. When app is backgrounded → network connections drop (OS suspends app)
2. When app resumes → automatically reconnects to peers via `reconnect_peers`
3. Clipboard only syncs when app is in foreground
4. **Pairing requests** show notifications (requires user action, has timeout)

### Platform Behavior (identical):

| Aspect                | Android & iOS                    |
|-----------------------|----------------------------------|
| Background execution  | App suspended by OS              |
| Network in background | Connections drop when suspended  |
| Clipboard sync        | Only when app is in foreground   |
| Pairing notifications | Yes (requires immediate action)  |

### Key files:
- `src-tauri/src/lib.rs` - Rust-side background detection and pending clipboard handling
- `src-tauri/src/state.rs` - `PendingClipboard` struct and `is_foreground` tracking
- `src/main.ts` - Visibility change listener that triggers `reconnect_peers`
- Android: `src-tauri/gen/android/app/src/main/java/com/decentpaste/application/MainActivity.kt`
- iOS: Uses standard Tauri lifecycle events (no native Swift code needed)

### Permissions:
- **Android**: `INTERNET`, `POST_NOTIFICATIONS` (for pairing requests only)
- **iOS**: No special permissions needed

### Commands:
- `reconnect_peers` - Called when app becomes visible to re-establish connections
- `process_pending_clipboard` - Called by frontend when app becomes visible
  - Returns: `{ content: string, from_device: string }` or `null`
  - Copies pending clipboard and clears the queue

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **Mobile clipboard (outgoing)** - Auto-monitoring disabled on Android/iOS; use "Share Clipboard" button
- **Mobile clipboard (incoming)** - Only syncs when app is in foreground; connections drop when backgrounded

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
