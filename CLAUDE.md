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

### Code Quality

```bash
# Format TypeScript/CSS
yarn format:fix

# Check Rust code
cd src-tauri && cargo check

# Clippy lints
cd src-tauri && cargo clippy
```

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

## Platform-Specific Notes

### Mobile

**Android:**
- **Clipboard outgoing**: Auto-monitoring disabled due to privacy restrictions. Two options:
  - Use system share sheet from any app → DecentPaste (via `tauri-plugin-decentshare`) — recommended
  - Use in-app "Share Now" button (requires clipboard access permission)
- **Clipboard incoming**: Only syncs when app is in foreground; connections drop when backgrounded
- When app resumes: automatically reconnects to peers via `reconnect_peers`

**iOS:**
- Basic Tauri iOS support exists but share extension not yet implemented
- Same foreground-only limitations as Android

### Android Share Intent

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
- `tauri-plugin-decentshare/android/.../DecentsharePlugin.kt` - Kotlin intent handler
- `src/main.ts` - `checkForPendingShare()` function
- `src-tauri/src/commands.rs` - `handle_shared_content` command

### Desktop

- Auto-updates via GitHub Releases (checks every 60 seconds)
- System tray icon with quick actions

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **Mobile background** - Network connections drop when app is backgrounded
- **iOS share extension** - Not yet implemented (Android share intent works via `tauri-plugin-decentshare`)

## See Also

- `ARCHITECTURE.md` - Detailed architecture documentation with data flow diagrams
- `SECURITY.md` - Security model, cryptographic stack, and threat considerations
- `README.md` - User-facing documentation
