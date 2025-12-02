# DecentPaste - Architecture Documentation

This document describes the DecentPaste project architecture for AI agents and developers who will work on this codebase.

## Project Overview

DecentPaste is a cross-platform clipboard sharing application that enables seamless clipboard synchronization between devices over a local network. It works similarly to Apple's Universal Clipboard but supports all platforms (Windows, macOS, Linux, Android, iOS).

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
- Handles incoming network events (peer discovery, messages)
- Processes commands from the main app (send clipboard, initiate pairing)
- Maintains peer connection state

#### `protocol.rs` - Message Types
Defines all protocol messages:
```rust
pub enum ProtocolMessage {
    Pairing(PairingMessage),    // Pairing flow messages
    Clipboard(ClipboardMessage), // Encrypted clipboard content
    Heartbeat(HeartbeatMessage), // Keep-alive
}
```

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
- `generate_shared_secret()` - Random 32-byte secret generation

#### `pairing.rs`
PIN-based pairing protocol:
1. Device A initiates pairing with Device B
2. Device B generates 6-digit PIN, displays to user
3. Both devices display PIN for visual verification
4. User confirms PIN matches on initiating device
5. Devices establish shared secret for encryption

#### `identity.rs`
- Generates unique device identity on first run
- Stores device ID, name, and keypair

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

#### `peers.rs` - PairedPeer
Stores paired device information:
- Peer ID, device name
- Shared secret (encrypted at rest)
- Pairing timestamp, last seen

### 5. Commands (`src/commands.rs`)

Tauri commands exposed to frontend:

| Command | Description |
|---------|-------------|
| `get_network_status` | Get current network status |
| `get_discovered_peers` | List discovered devices on network |
| `get_paired_peers` | List paired devices |
| `initiate_pairing` | Start pairing with a peer |
| `confirm_pairing` | Confirm PIN match |
| `get_clipboard_history` | Get clipboard history |
| `set_clipboard` | Set clipboard content |
| `get_settings` / `update_settings` | Manage app settings |
| `get_device_info` | Get this device's info |

### 6. Events (Emitted to Frontend)

| Event | Payload | Description |
|-------|---------|-------------|
| `network-status` | `NetworkStatus` | Network state changed |
| `peer-discovered` | `DiscoveredPeer` | New peer found |
| `peer-lost` | `string` (peer_id) | Peer went offline |
| `clipboard-received` | `ClipboardEntry` | Clipboard from peer |
| `pairing-request` | `{sessionId, peerId, deviceName}` | Incoming pairing |
| `pairing-pin` | `{sessionId, pin}` | PIN ready to display |
| `pairing-complete` | `{sessionId, peerId, deviceName}` | Pairing succeeded |

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

### Pairing Flow

```
Device A (Initiator)              Device B (Responder)
        │                                │
        │  1. User clicks "Pair"         │
        │────────────────────────────────>│
        │     PairingRequest              │
        │                                │
        │                                │ 2. Show pairing request UI
        │                                │    User clicks "Accept"
        │                                │
        │  3. PairingChallenge           │
        │<────────────────────────────────│
        │     {session_id, pin}           │
        │                                │
        │ 4. Display PIN: "123456"       │ 5. Display PIN: "123456"
        │                                │
        │ 6. User confirms PIN matches   │
        │────────────────────────────────>│
        │     PairingResponse             │
        │                                │
        │  7. PairingConfirm             │
        │<────────────────────────────────│
        │     {success, shared_secret}    │
        │                                │
        │ 8. Store pairing               │ 9. Store pairing
```

### Encryption

- All clipboard content is encrypted before transmission
- Each paired device pair shares a unique 256-bit secret
- AES-256-GCM provides authenticated encryption
- Content hash (SHA-256) is sent alongside for verification

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
3. **Single shared secret**: All paired peers currently use first peer's secret. Should use per-peer secrets.
4. **Mobile clipboard**: On Android/iOS, automatic clipboard monitoring is not supported. Users must manually share content using the "Share" button. Receiving clipboard from desktop works automatically.
5. **No persistence of clipboard history**: History is in-memory only.

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
- `libp2p` v0.54 - P2P networking
- `tauri-plugin-clipboard-manager` v2 - Cross-platform clipboard (including mobile)
- `aes-gcm` v0.10 - Encryption
- `tokio` v1 - Async runtime

### Frontend (Key Dependencies)
- `@tauri-apps/api` v2 - Tauri JavaScript API
- `tailwindcss` v4 - CSS framework
- `lucide` - Icons (inline SVGs)
