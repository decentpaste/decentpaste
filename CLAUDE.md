# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is DecentPaste?

DecentPaste is a cross-platform clipboard sharing app (like Apple's Universal Clipboard) that works on all platforms. It uses:

- **Tauri v2** for the desktop/mobile app
- **libp2p** for decentralized P2P networking
- **mDNS** for local network device discovery
- **X25519 ECDH** for secure key exchange during pairing
- **AES-256-GCM** for end-to-end encryption
- **IOTA Stronghold** for encrypted local storage (PIN-protected vault)

## Development Commands

### Quick Start

```bash
cd decentpaste-app
yarn install
yarn tauri dev
```

### Build & Development

```bash
# Desktop development
yarn tauri dev

# Build for production (desktop)
yarn tauri build

# Android development (requires Android SDK)
yarn tauri android dev

# Android production build
yarn tauri android build

# iOS development (macOS only)
yarn tauri ios dev

# iOS production build
yarn tauri ios build
```

### Code Quality & Testing

```bash
# Format TypeScript/CSS
cd decentpaste-app && yarn format:fix

# Check Rust code (compile check)
cd decentpaste-app/src-tauri && cargo check

# Run Clippy lints
cd decentpaste-app/src-tauri && cargo clippy

# Run full build (includes TypeScript compilation)
cd decentpaste-app && yarn build
```

**Note**: This project does not currently have automated tests. Manual testing involves running multiple instances (see below).

### Testing Multiple Instances

Run two instances on the same machine for pairing tests:

```bash
# Terminal 1
yarn tauri dev

# Terminal 2 (different port)
TAURI_DEV_PORT=1421 yarn tauri dev
```

## Claude Code Skills

Custom skills available via slash commands:

- `/bump-version` - Updates version across all config files (package.json, Cargo.toml, tauri.conf.json, downloads.json)
- `/android-release` - Builds, signs, and prepares Android APK/AAB for release
- `/github-release` - Creates a GitHub release with auto-generated release notes from commits since the last tag

## Project Structure

```
decentpaste/
├── decentpaste-app/              # Main Tauri application
│   ├── src/                      # Frontend (TypeScript + Tailwind v4)
│   │   ├── main.ts               # Entry point + share intent handling
│   │   ├── app.ts                # All UI (Dashboard, Peers, Settings, auth views)
│   │   ├── api/commands.ts       # Tauri command wrappers
│   │   ├── api/events.ts         # Event listeners
│   │   └── state/store.ts        # Reactive state
│   ├── src-tauri/src/            # Backend (Rust)
│   │   ├── lib.rs                # App initialization, spawns network & clipboard tasks
│   │   ├── commands.rs           # All Tauri command handlers
│   │   ├── state.rs              # AppState + flush helper methods
│   │   ├── network/              # libp2p (mDNS, gossipsub, request-response)
│   │   ├── clipboard/            # Polling monitor + echo prevention
│   │   ├── security/             # AES-GCM encryption, X25519 identity, PIN pairing
│   │   ├── vault/                # Stronghold encrypted storage lifecycle
│   │   └── storage/              # Settings & peer types
│   └── tauri-plugin-decentshare/ # Android "share with" plugin
│       ├── src/                  # Plugin Rust code
│       ├── android/              # Kotlin intent handler
│       └── guest-js/             # TypeScript bindings
├── website/                      # Landing page
└── .claude/skills/               # Claude Code skills
```

## Adding a New Tauri Command

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

## Adding a New Event

1. Emit from Rust:

```rust
app_handle.emit("my-event", payload)?;
```

2. Listen in `src/api/events.ts`:

```typescript
listen<MyPayload>('my-event', (e) => {
    // handle
});
```

## Key Architecture Concepts

### Clipboard Sync Flow

1. `ClipboardMonitor` polls every 500ms (configurable), hashes content with SHA-256
2. If hash differs from `last_hash` → encrypt separately for **each paired peer** using their specific shared secret
3. Broadcast via gossipsub (one message per peer)
4. Receiving peer checks `origin_device_id` (rejects own messages), decrypts, updates clipboard
5. Receiver calls `set_last_hash()` to prevent re-broadcasting received content

### Pairing (X25519 ECDH Key Exchange)

1. Device A sends `PairingRequest` with **X25519 public key**
2. Device B stores A's public key, generates 6-digit PIN
3. Device B sends `PairingChallenge` with PIN and **own public key**
4. Both devices display PIN for user to verify match
5. Both devices independently derive **same shared secret** via ECDH: `shared_secret = ECDH(my_private_key, peer_public_key)`
6. Stored encrypted in `vault.hold` (Stronghold)

**Security**: No secret is transmitted - both sides derive it from exchanged public keys.

### Vault & Authentication

All sensitive data is stored in an encrypted IOTA Stronghold vault, protected by a user PIN.

```
User PIN (4-8 digits)
       │
       ▼
┌─────────────────────────┐
│    Argon2id KDF         │ ← salt.bin (16 bytes, unique per install)
│  m=64MB, t=3, p=4       │
└─────────────────────────┘
       │
       ▼
   256-bit Key → vault.hold (paired_peers, clipboard_history, device_identity, libp2p_keypair)
```

**Vault States**: `NotSetup` → Onboarding wizard | `Locked` → PIN entry | `Unlocked` → Main app

### Flush-on-Write Pattern

Data is persisted **immediately** after every mutation via `AppState::flush_*()` methods, ensuring no data loss on unexpected termination.

### Device Name Broadcasting

Device names propagate through:
1. **libp2p identify** - On startup, name in `agent_version` field
2. **DeviceAnnounce** - Gossipsub message on name change in settings
3. **On connection** - Automatic announce when peer connects (catches offline peers)

## Platform-Specific Development

### Desktop (Windows, macOS, Linux)

- **Auto-updates**: Checks GitHub Releases every 60 seconds via Tauri updater plugin
- **System tray**: Quick actions for common tasks
- **Notifications**: Desktop notifications for clipboard events (configurable)
- **Single instance**: Only one app instance runs at a time (enforced by plugin)

### Mobile (Android & iOS)

**Android:**
- **Clipboard outgoing**: Auto-monitoring disabled due to privacy restrictions. Two options:
  - Use system share sheet from any app → DecentPaste (via `tauri-plugin-decentshare`) — recommended
  - Use in-app "Share Now" button (requires clipboard access permission)
- **Clipboard incoming**: Only syncs when app is in foreground; connections drop when backgrounded
- When app resumes: automatically reconnects to peers via `reconnect_peers`
- **Plugin build**: Build plugin JS bindings before running: `cd tauri-plugin-decentshare && yarn install && yarn build`

**iOS:**
- Basic Tauri iOS support exists but share extension not yet implemented
- Same foreground-only limitations as Android

### Android Share Intent (Plugin Architecture)

The `tauri-plugin-decentshare` enables sharing text directly from any Android app:

1. User selects text in any app → Share → DecentPaste
2. Plugin's `onNewIntent()` captures the shared text
3. Frontend polls for pending share via `getPendingShare()` command
4. If vault locked: stores in `pendingShare` state, processes after unlock
5. If vault unlocked: calls `handle_shared_content` which:
   - Triggers `ensure_connected()` with 3s timeout
   - Dials only disconnected peers (event-driven, no polling)
   - Returns honest status: "Sent to 2/3. 1 offline."

Key files:
- `tauri-plugin-decentshare/android/src/main/java/DecentsharePlugin.kt` - Kotlin intent handler
- `tauri-plugin-decentshare/android/src/main/AndroidManifest.xml` - Intent filter registration
- `decentpaste-app/src/main.ts` - `checkForPendingShare()` polling logic
- `decentpaste-app/src-tauri/src/commands.rs` - `handle_shared_content` command

**Plugin Development**: When modifying the plugin, rebuild JS bindings with `cd tauri-plugin-decentshare && yarn build`.

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **Mobile background** - Network connections drop when app is backgrounded
- **iOS share extension** - Not yet implemented (Android share intent works via `tauri-plugin-decentshare`)

## See Also

- `ARCHITECTURE.md` - Detailed architecture documentation with data flow diagrams
- `SECURITY.md` - Security model, cryptographic stack, and threat considerations
- `README.md` - User-facing documentation

## Critical Implementation Details

### Per-Peer Encryption
Clipboard content is encrypted **separately for each paired peer** using their specific shared secret. When broadcasting via gossipsub, the app sends one encrypted message per peer. Peer A cannot decrypt messages intended for Peer B.

### Echo Prevention
The `ClipboardMonitor` tracks `last_hash` (SHA-256) to prevent re-broadcasting received content. When a device receives clipboard from a peer, it updates its own `last_hash` to match, preventing an infinite broadcast loop.

### Flush-on-Write Pattern
**Always flush after mutating data**. The vault uses an immediate-persistence pattern:
- `AppState::flush_paired_peers()` - After pairing/unpairing
- `AppState::flush_device_identity()` - After device name change
- `AppState::flush_clipboard_history()` - After clipboard entry added

This prevents data loss on crashes/force-quit. Additionally, safety net flushes occur on:
- App backgrounded (mobile): `Focused(false)` event
- App exit (all platforms): `ExitRequested` event
- Manual vault lock: Before clearing encryption key from memory

### Consistent PeerId Across Restarts
The libp2p Ed25519 keypair is stored in the encrypted vault and loaded on startup. This ensures the device's PeerId remains consistent across app restarts, which is critical for pairing persistence.

### Event-Driven Reconnection (Mobile)
When the app resumes from background, `reconnect_peers` uses event-driven reconnection:
1. Identifies disconnected peers
2. Marks them as "Connecting" in UI
3. Dials peers and waits via `tokio::sync::Notify` (no polling)
4. Returns honest status: "Connected to 2/3 peers. 1 offline."

This is critical for mobile where network connections are killed when backgrounded.

### Device Name Propagation
Device names sync through three mechanisms to ensure peers always have the latest name:
1. **libp2p identify** - Startup, name in `agent_version` field
2. **DeviceAnnounce gossip** - On name change in settings
3. **On connection** - Automatic announce when peer connects (catches offline peers)

### Unpair → Rediscovery Flow
When unpairing a device:
1. Removes from `paired_peers` storage
2. Sends `RefreshPeer` command to NetworkManager
3. If peer still in `discovered_peers` cache, re-emits as discovered
4. Frontend shows device in "Discovered" section, ready for re-pairing
5. No app restart needed

### Auto-Update Target Selection
The updater dynamically selects the correct artifact based on:
- Platform (`@tauri-apps/plugin-os` platform())
- Architecture (`@tauri-apps/plugin-os` arch())
- Bundle type (`@tauri-apps/api/app` getBundleType())

For example, Linux can be `.deb`, `.rpm`, or `.appimage` - the updater matches the installation type.
