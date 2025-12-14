# DecentPaste - Architecture Documentation

This document describes the DecentPaste project architecture for AI agents and developers who will work on this
codebase.

## Project Overview

DecentPaste is a cross-platform clipboard sharing application that enables seamless clipboard synchronization between
devices over a local network. It works similarly to Apple's Universal Clipboard but supports all platforms (Windows,
macOS, Linux, Android, iOS).

### Key Features

- **Decentralized P2P networking** using libp2p (no central server)
- **Local network discovery** via mDNS
- **Secure clipboard sync** with AES-256-GCM encryption
- **PIN-based device pairing** for security
- **Automatic clipboard synchronization** when devices are paired

### Technology Stack

- **Backend**: Rust with Tauri v2
- **Frontend**: TypeScript with Tailwind CSS v4
- **Networking**: libp2p (mDNS, gossipsub, request-response)
- **Encryption**: AES-256-GCM, SHA-256
- **Key Exchange**: X25519 ECDH (Elliptic Curve Diffie-Hellman)

---

## Directory Structure

```
decentpaste/
├── Cargo.toml                    # Workspace root
├── Cargo.lock
├── ARCHITECTURE.md               # This file
└── decentpaste-app/              # Main Tauri application
    ├── package.json              # Frontend dependencies
    ├── tsconfig.json
    ├── vite.config.ts
    ├── postcss.config.js
    ├── index.html                # App entry HTML
    ├── src/                      # Frontend TypeScript
    │   ├── main.ts               # Entry point
    │   ├── app.ts                # Main application logic & UI
    │   ├── styles.css            # Tailwind CSS
    │   ├── api/
    │   │   ├── types.ts          # TypeScript interfaces
    │   │   ├── commands.ts       # Tauri command wrappers
    │   │   └── events.ts         # Event listener management
    │   ├── state/
    │   │   └── store.ts          # Reactive state store
    │   ├── components/
    │   │   └── icons.ts          # Lucide SVG icons
    │   └── utils/
    │       └── dom.ts            # DOM utilities
    └── src-tauri/                # Backend Rust
        ├── Cargo.toml            # Rust dependencies
        ├── tauri.conf.json       # Tauri configuration
        ├── capabilities/
        │   └── default.json      # Tauri permissions
        └── src/
            ├── main.rs           # Entry point
            ├── lib.rs            # Tauri app setup & initialization
            ├── commands.rs       # Tauri command handlers
            ├── state.rs          # Application state
            ├── error.rs          # Error types
            ├── network/          # libp2p networking
            │   ├── mod.rs
            │   ├── behaviour.rs  # Combined network behaviour
            │   ├── protocol.rs   # Message types
            │   ├── swarm.rs      # Network manager
            │   └── events.rs     # Network events
            ├── clipboard/        # Clipboard handling
            │   ├── mod.rs
            │   ├── monitor.rs    # Clipboard polling
            │   └── sync.rs       # Sync logic
            ├── security/         # Cryptography & pairing
            │   ├── mod.rs
            │   ├── crypto.rs     # AES-GCM encryption
            │   ├── identity.rs   # Device identity
            │   └── pairing.rs    # PIN pairing protocol
            └── storage/          # Persistence
                ├── mod.rs
                ├── config.rs     # App settings
                └── peers.rs      # Paired peers storage
```

---

## Architecture Overview

### Data Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                         FRONTEND (TypeScript)                        │
├─────────────────────────────────────────────────────────────────────┤
│  ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐      │
│  │Dashboard │    │ PeerList │    │ History  │    │ Settings │      │
│  └────┬─────┘    └────┬─────┘    └────┬─────┘    └────┬─────┘      │
│       │               │               │               │             │
│       └───────────────┴───────────────┴───────────────┘             │
│                               │                                      │
│                        ┌──────┴──────┐                              │
│                        │    Store    │  (Reactive State)            │
│                        └──────┬──────┘                              │
│                               │                                      │
│              ┌────────────────┼────────────────┐                    │
│              │                │                │                    │
│       ┌──────┴──────┐  ┌──────┴──────┐  ┌──────┴──────┐            │
│       │  Commands   │  │   Events    │  │   Types     │            │
│       │  (invoke)   │  │  (listen)   │  │             │            │
│       └──────┬──────┘  └──────┬──────┘  └─────────────┘            │
└──────────────┼────────────────┼─────────────────────────────────────┘
               │   Tauri IPC    │
┌──────────────┼────────────────┼─────────────────────────────────────┐
│              │                │                                      │
│       ┌──────┴──────┐  ┌──────┴──────┐      BACKEND (Rust)          │
│       │  Commands   │  │   Events    │                              │
│       │  Handler    │  │   Emitter   │                              │
│       └──────┬──────┘  └──────┬──────┘                              │
│              │                │                                      │
│              └────────┬───────┘                                      │
│                       │                                              │
│                ┌──────┴──────┐                                       │
│                │  AppState   │                                       │
│                └──────┬──────┘                                       │
│                       │                                              │
│       ┌───────────────┼───────────────┬───────────────┐             │
│       │               │               │               │             │
│ ┌─────┴─────┐  ┌──────┴──────┐  ┌─────┴─────┐  ┌─────┴─────┐       │
│ │  Network  │  │  Clipboard  │  │ Security  │  │  Storage  │       │
│ │  Manager  │  │   Monitor   │  │           │  │           │       │
│ └─────┬─────┘  └──────┬──────┘  └───────────┘  └───────────┘       │
│       │               │                                              │
│       │         ┌─────┴─────┐                                       │
│       │         │  Tauri    │  (Clipboard Plugin)                   │
│       │         └───────────┘                                       │
│       │                                                              │
│ ┌─────┴──────────────────────────────────────────────────────┐      │
│ │                     libp2p Swarm                            │      │
│ │  ┌─────────┐  ┌───────────┐  ┌─────────────────┐  ┌──────┐ │      │
│ │  │  mDNS   │  │ Gossipsub │  │ Request-Response│  │ ID   │ │      │
│ │  │Discovery│  │ (Broadcast)│  │    (Pairing)   │  │      │ │      │
│ │  └─────────┘  └───────────┘  └─────────────────┘  └──────┘ │      │
│ └────────────────────────────────────────────────────────────┘      │
│                              │                                       │
│                         Local Network                                │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Backend Components

### 1. Network Layer (`src/network/`)

#### `behaviour.rs` - DecentPasteBehaviour

Combined libp2p network behaviour with four sub-behaviours:

- **mDNS**: Automatic local network peer discovery
- **Gossipsub**: Pub/sub for broadcasting clipboard content to all paired peers
- **Request-Response**: Point-to-point messaging for pairing protocol
- **Identify**: Peer identification and metadata exchange

```rust
#[derive(NetworkBehaviour)]
pub struct DecentPasteBehaviour {
    pub mdns: mdns::tokio::Behaviour,
    pub gossipsub: gossipsub::Behaviour,
    pub request_response: request_response::Behaviour<DecentPasteCodec>,
    pub identify: identify::Behaviour,
}
```

#### `swarm.rs` - NetworkManager

Manages the libp2p swarm lifecycle:

- Accepts a persisted keypair for consistent PeerId across restarts
- Handles incoming network events (peer discovery, messages)
- Processes commands from the main app (send clipboard, initiate pairing)
- Maintains peer connection state
- Filters out already-paired peers from discovery events
- **Connection resilience**: Automatically retries failed connections (3 attempts, 2s delay)
- **Gossipsub optimization**: Adds peers to explicit peer list on connection for immediate mesh inclusion
- **Mobile support**: Handles `ReconnectPeers` command for app resume from background
- **Device name tracking**: Stores current device name and announces it on new connections
- **Peer refresh**: Re-emits discovered peers after unpairing so they can be paired again

#### `protocol.rs` - Message Types

Defines all protocol messages:

```rust
pub enum ProtocolMessage {
    Pairing(PairingMessage),        // Pairing flow messages
    Clipboard(ClipboardMessage),     // Encrypted clipboard content
    Heartbeat(HeartbeatMessage),     // Keep-alive
    DeviceAnnounce(DeviceAnnounceMessage), // Device name broadcasts
}
```

The `DeviceAnnounce` message is broadcast via gossipsub when:
- Device name changes in settings
- A new peer connects (to catch up peers that were offline)

### 2. Clipboard Layer (`src/clipboard/`)

#### `monitor.rs` - ClipboardMonitor

- Polls system clipboard every 500ms (configurable)
- Hashes content with SHA-256 to detect changes
- Emits `ClipboardChange` events when content changes

#### `sync.rs` - SyncManager

- Prevents echo/loops by tracking recent content hashes
- Implements last-write-wins conflict resolution
- Manages clipboard entry history

### 3. Security Layer (`src/security/`)

#### `crypto.rs`

- `encrypt_content()` / `decrypt_content()` - AES-256-GCM encryption
- `hash_content()` - SHA-256 hashing for deduplication

#### `identity.rs`

- Generates unique device identity with **X25519 keypair** on first run
- `derive_shared_secret()` - ECDH key derivation using X25519
- Stores device ID, name, and keypair (public + private)

#### `pairing.rs`

PIN-based pairing protocol with X25519 key exchange:

1. Device A initiates pairing, sends **public key** in request
2. Device B generates 6-digit PIN, responds with **own public key**
3. Both devices display PIN for visual verification
4. User confirms PIN matches on initiating device
5. Both devices independently derive the **same shared secret** using ECDH:
   - Device A: `shared = ECDH(A_private, B_public)`
   - Device B: `shared = ECDH(B_private, A_public)`
6. No secret is transmitted over the network - only public keys

### 4. Storage Layer (`src/storage/`)

#### `config.rs` - AppSettings

```rust
pub struct AppSettings {
    pub device_name: String,
    pub auto_sync_enabled: bool,
    pub clipboard_history_limit: usize,
    pub clear_history_on_exit: bool,
    pub show_notifications: bool,
    pub clipboard_poll_interval_ms: u64,
}
```

#### `peers.rs` - PairedPeer & Identity Persistence

Stores paired device information:

- Peer ID, device name
- Shared secret (encrypted at rest)
- Pairing timestamp, last seen

Also handles **libp2p keypair persistence**:

- `get_or_create_libp2p_keypair()` - Loads or creates the libp2p Ed25519 keypair
- Stored in `libp2p_keypair.bin` using protobuf encoding
- Ensures consistent PeerId across app restarts (critical for pairing to work)

### 5. Commands (`src/commands.rs`)

Tauri commands exposed to frontend:

| Command                            | Description                                                               |
|------------------------------------|---------------------------------------------------------------------------|
| `get_network_status`               | Get current network status                                                |
| `get_discovered_peers`             | List discovered devices (excludes already-paired devices)                 |
| `get_paired_peers`                 | List paired devices                                                       |
| `remove_paired_peer`               | Unpair a device (re-emits as discovered if still online)                  |
| `initiate_pairing`                 | Start pairing with a peer                                                 |
| `respond_to_pairing`               | Accept/reject incoming pairing request                                    |
| `confirm_pairing`                  | Confirm PIN match after user verification                                 |
| `cancel_pairing`                   | Cancel an active pairing session                                          |
| `get_clipboard_history`            | Get clipboard history                                                     |
| `set_clipboard`                    | Set clipboard content                                                     |
| `share_clipboard_content`          | Manually share clipboard with peers (for mobile)                          |
| `clear_clipboard_history`          | Clear all clipboard history                                               |
| `reconnect_peers`                  | Force reconnection to all discovered peers (for mobile background resume) |
| `get_settings` / `update_settings` | Manage app settings (broadcasts device name change)                       |
| `get_device_info`                  | Get this device's info                                                    |
| `get_pairing_sessions`             | Get active pairing sessions                                               |

### 6. Events (Emitted to Frontend)

| Event                | Payload                           | Description                                  |
|----------------------|-----------------------------------|-------------------------------------------------|
| `network-status`     | `NetworkStatus`                   | Network state changed                        |
| `peer-discovered`    | `DiscoveredPeer`                  | New peer found                               |
| `peer-lost`          | `string` (peer_id)                | Peer went offline                            |
| `peer-name-updated`  | `{peerId, deviceName}`            | Peer's device name changed (via DeviceAnnounce) |
| `clipboard-received` | `ClipboardEntry`                  | Clipboard from peer                          |
| `pairing-request`    | `{sessionId, peerId, deviceName}` | Incoming pairing request                     |
| `pairing-pin`        | `{sessionId, pin}`                | PIN ready to display                         |
| `pairing-complete`   | `{sessionId, peerId, deviceName}` | Pairing succeeded                            |

---

## Frontend Components

### State Management (`src/state/store.ts`)

Simple reactive store with subscriptions:

```typescript
interface AppState {
    networkStatus: NetworkStatus;
    discoveredPeers: DiscoveredPeer[];
    pairedPeers: PairedPeer[];
    clipboardHistory: ClipboardEntry[];
    currentView: 'dashboard' | 'peers' | 'history' | 'settings';
    settings: AppSettings;
    deviceInfo: DeviceInfo | null;
    // ... UI state
}
```

### Views (`src/app.ts`)

Single-file application with four main views:

1. **Dashboard**: Quick stats, recent clipboard items, quick actions
2. **Peers**: Discovered and paired devices, pairing UI
3. **History**: Full clipboard history with copy actions
4. **Settings**: Device name, sync preferences, history settings

### API Layer (`src/api/`)

- `commands.ts`: Typed wrappers for all Tauri commands
- `events.ts`: Event listener management with typed handlers
- `types.ts`: TypeScript interfaces matching Rust types

### Entry Point (`src/main.ts`)

- Initializes the app on DOMContentLoaded
- **Mobile background handling**: Listens for `visibilitychange` events and triggers `reconnectPeers()` when app becomes
  visible (critical for mobile where connections drop when backgrounded)

---

## Key Concepts

### Clipboard Sync Flow

1. User copies text on Device A
2. ClipboardMonitor detects change, computes hash
3. SyncManager checks if content is new (not from another device)
4. Content is encrypted with shared secret
5. Encrypted message broadcast via gossipsub
6. Device B receives message, decrypts with shared secret
7. Device B's clipboard is updated
8. SyncManager records hash to prevent echo

### Pairing Flow (with X25519 ECDH Key Exchange)

```
Device A (Initiator)              Device B (Responder)
        │                                │
        │  1. User clicks "Pair"         │
        │────────────────────────────────>│
        │     PairingRequest              │
        │     {session_id, device_name,   │
        │      public_key: A_pub}         │
        │                                │
        │                                │ 2. Show pairing request UI
        │                                │    Store A_pub for ECDH
        │                                │    User clicks "Accept"
        │                                │    Generate PIN
        │                                │
        │  3. PairingChallenge           │
        │<────────────────────────────────│
        │     {session_id, pin,           │
        │      device_name,               │
        │      public_key: B_pub}         │
        │                                │
        │ 4. Store B_pub for ECDH        │
        │    Display PIN: "123456"       │ 5. Display PIN: "123456"
        │    User confirms PIN            │    (waiting for initiator)
        │                                │
        │ 6. Derive shared_secret =      │
        │    ECDH(A_priv, B_pub)          │
        │    PairingConfirm              │
        │────────────────────────────────>│
        │    {session_id, success,        │
        │     shared_secret, device_name} │
        │                                │
        │                                │ 7. Derive shared_secret =
        │                                │    ECDH(B_priv, A_pub)
        │                                │    Verify: derived == received
        │  8. PairingConfirm (ack)       │
        │<────────────────────────────────│
        │     {success}                   │
        │                                │
        │ 9. Store pairing               │ 10. Store pairing
```

**Security**: Both devices derive the same shared secret independently using ECDH.
The secret sent in step 6 is for verification only - Device B also derives it locally
and compares. Even if intercepted, an attacker cannot derive the secret without
a private key.

### Encryption

- All clipboard content is encrypted before transmission
- Each paired device pair shares a unique 256-bit secret (derived via X25519 ECDH)
- AES-256-GCM provides authenticated encryption
- Content hash (SHA-256) is sent alongside for verification
- **Per-peer encryption**: Messages are encrypted separately for each paired peer using their specific shared secret

### Device Name Broadcasting

Device names are synchronized across peers through multiple channels:

**At Startup:**
- Device name is included in libp2p identify protocol's `agent_version` field
- Format: `decentpaste/<version>/<device_name>`
- Peers parsing identify can extract the custom device name

**On Settings Change:**
- When user changes device name in Settings, `update_settings` broadcasts a `DeviceAnnounce` message
- All connected peers receive it via gossipsub and update their peer lists (both discovered and paired)
- Peers also save the updated name to persistent storage

**On New Connection:**
- When a new peer connects, NetworkManager automatically broadcasts a `DeviceAnnounce` message
- This handles the case where:
  - Device A changes name while Device B is offline
  - Device B comes back online and connects
  - Device B immediately receives Device A's current name
- Ensures devices always have the latest name without requiring app restart

### Unpair → Rediscovery

When a paired device is unpaired:
1. `remove_paired_peer` command removes it from `paired_peers` storage
2. Sends `RefreshPeer` command to NetworkManager
3. NetworkManager checks if the peer is still in its `discovered_peers` cache
4. If found, re-emits a `PeerDiscovered` event
5. Frontend updates and shows the device in the "Discovered Devices" section
6. User can pair with the device again immediately without restart

---

## Configuration Files

### `src-tauri/tauri.conf.json`

- App metadata (name, version, identifier)
- Window configuration (size, title)
- Build commands

### `src-tauri/capabilities/default.json`

- Tauri permissions for the main window
- Enables core events and opener plugin

### `postcss.config.js`

- Uses `@tailwindcss/postcss` for Tailwind v4

### Data Directory (`~/.local/share/com.decentpaste.app/`)

- `identity.json` - Device identity (device_id, device_name, public_key)
- `private_key.bin` - Device private key (restricted permissions)
- `libp2p_keypair.bin` - libp2p Ed25519 keypair for consistent PeerId
- `peers.json` - Paired peers with shared secrets
- `settings.json` - App settings

---

## Development Commands

```bash
# Install dependencies
cd decentpaste-app
yarn install

# Run in development mode
yarn tauri dev

# Build for production
yarn tauri build

# Check Rust code
cd src-tauri && cargo check

# Build frontend only
yarn build
```

---

## Known Limitations & Future Work

1. **Text-only clipboard**: Currently only supports text. Images/files could be added.
2. **Local network only**: Uses mDNS, so devices must be on same network. Internet relay could be added.
3. **Mobile clipboard**: On Android/iOS, automatic clipboard monitoring is not supported. Users must manually share
   content using the "Share" button. Receiving clipboard from desktop works automatically.
4. **No persistence of clipboard history**: History is in-memory only.
5. **Plaintext secret storage**: Shared secrets are stored in `peers.json` without OS keychain integration.
6. **Device name in identify**: The identify protocol includes device name in `agent_version` field, which is
   cosmetic (for human readability) but not ideal. Custom TXT records in mDNS would be better but more complex.

---

## Troubleshooting

### Network Issues

- Check that devices are on the same local network
- Verify mDNS is not blocked by firewall
- Check network status in app UI

### Pairing Issues

- Ensure both devices have the app running
- Verify PIN matches exactly on both devices
- Check for pairing timeout (5 minutes)

### Clipboard Not Syncing

- Verify devices are paired (check Peers view)
- Ensure auto-sync is enabled in Settings
- Check that content is text (images not supported)

---

## Dependencies

### Rust (Key Dependencies)

- `tauri` v2 - Application framework
- `libp2p` v0.56 - P2P networking
- `tauri-plugin-clipboard-manager` v2 - Cross-platform clipboard (including mobile)
- `aes-gcm` v0.10 - AES-256-GCM encryption
- `x25519-dalek` v2 - X25519 ECDH key exchange
- `tokio` v1 - Async runtime

### Frontend (Key Dependencies)

- `@tauri-apps/api` v2 - Tauri JavaScript API
- `@tauri-apps/plugin-clipboard-manager` v2 - Clipboard access for mobile
- `tailwindcss` v4 - CSS framework
- `lucide` - Icons (inline SVGs)
