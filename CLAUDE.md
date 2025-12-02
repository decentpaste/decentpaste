# DecentPaste - AI Agent Quick Reference

## What is this project?

DecentPaste is a cross-platform clipboard sharing app (like Apple's Universal Clipboard) that works on all platforms. It uses:
- **Tauri v2** for the desktop/mobile app
- **libp2p** for decentralized P2P networking
- **mDNS** for local network device discovery
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
    ├── commands.rs         # Tauri command handlers (17 commands)
    ├── network/            # libp2p networking
    │   ├── behaviour.rs    # mDNS + gossipsub + request-response
    │   └── swarm.rs        # Network manager
    ├── clipboard/          # Clipboard handling
    │   ├── monitor.rs      # Polls clipboard every 500ms
    │   └── sync.rs         # Deduplication logic
    ├── security/           # Encryption & pairing
    │   ├── crypto.rs       # AES-256-GCM
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
2. If content changed & is local → encrypt with shared secret
3. Broadcast via gossipsub to all peers
4. Receiving peer decrypts, updates clipboard
5. Hash tracked to prevent echo loops

## How Pairing Works

1. Device A sends `PairingRequest`
2. Device B shows 6-digit PIN to user
3. User confirms PIN matches on both devices
4. Devices exchange encrypted shared secret
5. Stored in `~/.local/share/com.decentpaste.app/peers.json`

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
  return invoke('my_command', { arg });
}
```

### Add a new event

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

## Current Limitations

- **Text only** - No image/file support yet
- **Local network only** - mDNS doesn't work across networks
- **Single shared secret** - Currently uses first peer's secret for all (bug)
- **In-memory history** - Not persisted to disk
- **Mobile clipboard** - Auto-monitoring disabled on Android/iOS; use "Share Clipboard" button on Dashboard

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
