# CLAUDE.md

Guidance for Claude Code when working with this repository.

## What is DecentPaste?

Cross-platform clipboard sharing app (like Apple's Universal Clipboard) for all platforms:

- **Tauri v2** - Desktop/mobile app framework
- **libp2p** - Decentralized P2P networking
- **mDNS** - Local network device discovery
- **X25519 ECDH** - Secure key exchange during pairing
- **AES-256-GCM** - End-to-end encryption
- **Hardware Security** - Platform-native key storage (TEE/Keychain) with PIN fallback

## Development Commands

```bash
# Quick start
cd decentpaste-app && yarn install && yarn tauri dev

# Desktop
yarn tauri dev              # Development
yarn tauri build            # Production

# Mobile (requires Android SDK / Xcode)
yarn tauri android dev      # Android dev
yarn tauri android build    # Android production
yarn tauri ios dev          # iOS dev (macOS only)
yarn tauri ios build        # iOS production

# Code quality
cd decentpaste-app && yarn format:fix                    # Format TS/CSS
cd decentpaste-app/src-tauri && cargo check              # Rust compile check
cd decentpaste-app/src-tauri && cargo clippy             # Rust lints
cd decentpaste-app && yarn build                         # Full TS build
```

**Testing**: No automated tests. Manual testing = run desktop + mobile instances together for pairing tests.

## Git Commits

Use **Conventional Commits**: `<type>(<scope>): <description>`

**Types:** `feat` | `fix` | `refactor` | `docs` | `chore` | `style`

Examples: `feat(vault): add biometric auth` | `fix(clipboard): prevent echo loop` | `refactor(network): simplify reconnection`

## Code Style

### TypeScript

- Single quotes, 120 char width, trailing commas
- Named imports: `import { invoke } from '@tauri-apps/api/core'`
- `interface` for objects, `type` for unions/primitives
- Variables/functions: `camelCase` | Classes: `PascalCase` | Constants: `UPPER_SNAKE_CASE`
- Wrap Tauri commands in try/catch, use `getErrorMessage()` for user-facing errors
- State: `store.set(key, value)`, `store.subscribe(key, listener)`
- Command wrappers in `api/commands.ts` mirror Rust `commands.rs`

### Rust

- 4-space indent, trailing commas in multi-line definitions
- Imports: std lib → external crates → local modules
- All structs derive `Debug`; serializable types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Variables/functions: `snake_case` | Types: `PascalCase` | Constants: `SCREAMING_SNAKE_CASE`
- Use `DecentPasteError` enum with `thiserror`, propagate with `?`
- Async with `tokio`, channels for inter-task communication
- Logging: `tracing` crate (`debug!`, `info!`, `warn!`, `error!`)
- Commands: `#[tauri::command]`, accept `State<'_, AppState>`, return `Result<T>`
- Minimal comments - only for non-obvious behavior or security-critical code

## Project Structure

```
decentpaste-app/
├── src/                          # Frontend (TypeScript + Tailwind v4)
│   ├── main.ts                   # Entry point + share intent handling
│   ├── app.ts                    # All UI views
│   ├── api/commands.ts           # Tauri command wrappers
│   ├── api/events.ts             # Event listeners
│   └── state/store.ts            # Reactive state
├── src-tauri/src/                # Backend (Rust)
│   ├── lib.rs                    # App init, spawns network & clipboard tasks
│   ├── commands.rs               # All Tauri command handlers
│   ├── state.rs                  # AppState + flush helpers
│   ├── network/                  # libp2p (mDNS, gossipsub, request-response)
│   ├── clipboard/                # Polling monitor + echo prevention
│   ├── security/                 # AES-GCM, X25519, PIN pairing
│   ├── vault/                    # Encrypted vault (hardware-backed or PIN)
│   └── storage/                  # Settings & peer types
└── tauri-plugin-decentshare/     # Android/iOS share plugin
```

## Adding a New Tauri Command

1. **Rust** (`src-tauri/src/commands.rs`):
```rust
#[tauri::command]
pub async fn my_command(state: State<'_, AppState>, arg: String) -> Result<String> {
    // implementation
}
```

2. **Register** (`src-tauri/src/lib.rs`):
```rust
.invoke_handler(tauri::generate_handler![commands::my_command])
```

3. **TypeScript** (`src/api/commands.ts`):
```typescript
export async function myCommand(arg: string): Promise<string> {
    return invoke('my_command', { arg });
}
```

## Adding a New Event

1. **Emit from Rust**: `app_handle.emit("my-event", payload)?;`
2. **Listen in TS** (`src/api/events.ts`): `listen<MyPayload>('my-event', (e) => { ... });`

## Key Patterns

| Pattern                   | Description                                                                       |
|---------------------------|-----------------------------------------------------------------------------------|
| **Flush-on-Write**        | Always call `flush_*()` immediately after mutating vault data                     |
| **Per-Peer Encryption**   | Clipboard encrypted separately for each peer using their unique shared secret     |
| **Echo Prevention**       | `ClipboardMonitor` tracks `last_hash` to prevent re-broadcasting received content |
| **Event-Driven**          | Rust emits → Frontend listens. Use `tokio::sync::Notify` (no polling)             |
| **Platform Conditionals** | `#[cfg(desktop)]` / `#[cfg(mobile)]` for platform-specific code                   |

## Platform Notes

**Desktop**: Auto-updates (GitHub Releases), system tray, notifications, single instance.

**Mobile (Android/iOS)**:
- Clipboard outgoing: Use system share sheet → DecentPaste (auto-monitoring disabled)
- Clipboard incoming: Only syncs in foreground; connections drop when backgrounded
- On resume: `reconnect_peers` automatically redials disconnected peers

## Creating Tauri Plugins

```bash
npx @tauri-apps/cli plugin new --android --ios <plugin-name>
```

- **Plugin name**: Prefix with `decent` (e.g., `decentshare`, `decentsecret`)
- **Package name**: `com.decentpaste.plugins.<plugin-name>`

## Claude Code Skills

**Release workflow:**
- `/bump-version` - Update version across all config files
- `/android-release` - Build, sign, prepare Android APK/AAB
- `/ios-release` - Build, archive, prepare iOS for TestFlight
- `/github-release` - Create GitHub release with auto-generated notes

**iOS fixes:**
- `/ios-icon-fix` - Remove alpha channel from iOS icons (App Store requirement)
- `/appstore-screenshots` - Resize screenshots to App Store dimensions

**General:**
- `/frontend-design` - Create distinctive, production-grade frontend interfaces
- `/skill-creator` - Guide for creating new skills

## Documentation Guidelines

**When to document**: Architecture changes → `ARCHITECTURE.md` | Security changes → `SECURITY.md` | New workflows → `CLAUDE.md`

**When NOT to**: Minor bug fixes, trivial details, self-documenting code.

**Diagrams**: Use Mermaid (not ASCII art).

## Current Limitations

- **Text only** - No image/file support
- **Local network only** - mDNS doesn't work across networks
- **Mobile background** - Network drops when backgrounded

## See Also

- `ARCHITECTURE.md` - Detailed architecture, data flows, component docs
- `SECURITY.md` - Cryptographic stack, threat model, pairing protocol
