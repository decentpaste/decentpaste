# AGENTS.md

This file provides coding guidelines and commands for agentic AI assistants working on DecentPaste.

---

## Build, Lint, and Typecheck Commands

```bash
# Development
cd decentpaste-app && yarn tauri dev

# Production build
cd decentpaste-app && yarn tauri build

# Rust compile check
cd decentpaste-app/src-tauri && cargo check

# Rust linting (Clippy)
cd decentpaste-app/src-tauri && cargo clippy

# Full TypeScript compilation
cd decentpaste-app && yarn build  # includes `tsc`

# Format TypeScript/CSS
cd decentpaste-app && yarn format:fix
```

**Note:** This project does not have automated tests. Manual testing involves running desktop and mobile instances together (same app supports single-instance per platform):
```bash
# Terminal 1: Desktop
yarn tauri dev

# Terminal 2: Mobile (requires Android/iOS setup)
yarn tauri android dev  # or: yarn tauri ios dev
```

---

## TypeScript Code Style

### Formatting
- Single quotes (`'`)
- 120 character line width
- Trailing commas in objects/arrays
- Prettier auto-formats via `yarn format:fix`

### Imports
- Named imports preferred: `import { invoke } from '@tauri-apps/api/core'`
- External Tauri APIs: use `@tauri-apps/api` and `@tauri-apps/plugin-*`
- Local imports: relative paths, organized by module
- Order: external packages, local imports, then type imports

### Types
- All data structures use `interface` for objects, `type` for unions/primitives
- Match Rust types in `api/types.ts` with backend `storage/peers.rs`, `network/protocol.rs`
- Strict TypeScript enabled (`strict: true` in tsconfig.json)

### Naming Conventions
- Variables/functions: `camelCase`
- Classes: `PascalCase`
- Constants: `UPPER_SNAKE_CASE`
- Files: `kebab-case.ts` for modules

### Error Handling
- Wrap Tauri commands in try/catch
- Use `getErrorMessage()` helper for user-facing error messages
- Log errors to console for debugging

### State Management
- Use `store.set(key, value)` and `store.get(key)`
- Subscribe to changes via `store.subscribe(key, listener)`
- Mutate arrays immutably: `update('key', (arr) => [...arr, newItem])`

### UI/Components
- DOM manipulation via helper functions in `utils/dom.ts` (`$`, `escapeHtml`)
- Inline SVG icons from `components/icons.ts` (Lucide)
- Delegated event listeners on root element for performance

### Tauri Commands
- Command wrappers in `api/commands.ts` mirror Rust `commands.rs`
- Use snake_case command names: `get_network_status` → `invoke('get_network_status')`
- Event listeners in `api/events.ts` with typed handlers

---

## Rust Code Style

### Formatting
- 4-space indentation (standard rustfmt)
- Trailing commas in multi-line struct/enum definitions

### Imports
- Standard library first, then external crates, then local modules
- Group related imports together
- Use `use crate::module::Type` for internal imports

### Types
- All structs/enums derive `Debug`
- Serializable types: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Use `serde` for JSON serialization
- Public types exported at `storage/peers.rs`, `network/protocol.rs`

### Naming Conventions
- Variables/functions: `snake_case`
- Types/structs/enums: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case` (files and directories)

### Error Handling
- Use custom `DecentPasteError` enum from `error.rs` with `thiserror`
- Type alias: `pub type Result<T> = std::result::Result<T, DecentPasteError>`
- Convert third-party errors with `#[from]` attribute where appropriate
- Use `?` operator for error propagation

### Async/Await
- All async functions use `tokio` runtime
- Spawn tasks with `tokio::spawn()` for concurrent operations
- Use channels (`mpsc`, `broadcast`) for inter-task communication

### Logging
- Use `tracing` crate: `debug!()`, `info!()`, `warn!()`, `error!()`
- Log levels: `decentpaste_app=debug,libp2p=info` (default env filter)

### Tauri Commands
- Commands in `commands.rs` with `#[tauri::command]` attribute
- Accept `State<'_, AppState>` for shared state
- Return `Result<T>` where `T` is `Serialize`
- Emit events via `app_handle.emit("event-name", payload)?`

### State Management
- `AppState` holds all shared application state
- Use `Arc<RwLock<T>>` for thread-safe shared state
- Flush vault data immediately after mutations (`flush_*()` methods)

### Module Documentation
- Add module-level doc comments: `//! Module description`
- Include important patterns and security notes in docs

### Comments
- Minimal comments (code should be self-documenting)
- Only comment non-obvious behavior, security-critical operations, or complex logic

---

## Documentation Guidelines

### Diagrams
- Use **Mermaid diagrams** instead of ASCII art or text descriptions for:
  - Data flows
  - Architecture diagrams
  - Sequence diagrams
  - State transitions
- Mermaid renders properly in GitHub and Markdown viewers
- Example: See ARCHITECTURE.md and SECURITY.md for Mermaid diagram usage

---

## Tauri Plugins

### Creating New Plugins
Create cross-platform plugins for DecentPaste with:
```bash
npx @tauri-apps/cli plugin new --android --ios <plugin name>
```

**Note:** Run this command in a separate terminal tab to avoid conflicts with any running development server.

### Plugin Naming Convention
- **Plugin name**: Prefix with `decent` followed by nature of plugin (e.g., `decentshare`, `decentsecret`)
- **Package name**: Use format `com.decentpaste.plugins.<plugin name>` (e.g., `com.decentpaste.plugins.decentshare`)

---

## Project-Specific Patterns

### Flush-on-Write Pattern
ALWAYS call flush methods immediately after mutating sensitive data:
- `AppState::flush_paired_peers()` - after pairing/unpairing
- `AppState::flush_device_identity()` - after device name change
- `AppState::flush_clipboard_history()` - after clipboard entry added

### Per-Peer Encryption
Clipboard content is encrypted **separately for each paired peer** using their specific shared secret. Messages for Peer A cannot be decrypted by Peer B.

### Event-Driven Architecture
- Rust emits events → Frontend listens via `api/events.ts`
- Use `tokio::sync::Notify` for async coordination (no polling)
- Channel send/receive for inter-module communication

### Platform Conditionals
- `#[cfg(desktop)]` / `#[cfg(mobile)]` for platform-specific code
- Desktop plugins: `notification`, `single-instance`, `autostart`
- Mobile-only: Share intent via `tauri-plugin-decentshare`

### Security
- Never log sensitive data (PINs, keys, shared secrets)
- PIN never stored; only Argon2id-derived encryption key in memory
- Vault cleared from memory on lock
- X25519 ECDH for key exchange (shared secret never transmitted)
