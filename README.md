# DecentPaste

**Universal Clipboard for Every Device** — A cross-platform clipboard sharing app that works like Apple's Universal Clipboard, but for all platforms.

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri%20v2-24C8D8.svg)](https://tauri.app)
[![libp2p](https://img.shields.io/badge/Powered%20by-libp2p-blue.svg)](https://libp2p.io)

<p align="center">
  <img src="website/assets/og-image.png" alt="DecentPaste Screenshot" width="600">
</p>

## What is DecentPaste?

DecentPaste lets you seamlessly share your clipboard between all your devices over your local network. Copy on your laptop, paste on your phone. No cloud servers, no accounts, no subscriptions — just secure, peer-to-peer clipboard sync.

### Key Features

- **Cross-Platform** — Works on Windows, macOS, Linux, Android, and iOS
- **Decentralized** — No central server; devices connect directly via P2P
- **Auto-Discovery** — Devices find each other automatically on your local network
- **End-to-End Encrypted** — AES-256-GCM encryption; only paired devices can read your clipboard
- **Secure Pairing** — 6-digit PIN verification with X25519 key exchange
- **Lightweight** — Small binary size (~15MB) thanks to Tauri
- **Open Source** — Apache 2.0 licensed

## How It Works

```
┌──────────────┐                              ┌──────────────┐
│   Device A   │                              │   Device B   │
│              │      Local Network (mDNS)    │              │
│  1. Copy     │  ──────────────────────────► │              │
│     text     │      Auto-discovery          │              │
│              │                              │              │
│  2. Encrypt  │      Encrypted via           │  3. Decrypt  │
│     with     │  ──────────────────────────► │     with     │
│     shared   │      gossipsub (P2P)         │     shared   │
│     secret   │                              │     secret   │
│              │                              │              │
│              │                              │  4. Paste!   │
└──────────────┘                              └──────────────┘
```

1. **Discovery**: Devices find each other using mDNS (like AirDrop/Chromecast)
2. **Pairing**: One-time secure pairing with PIN verification establishes a shared secret
3. **Sync**: Clipboard changes are encrypted and broadcast to paired devices
4. **Receive**: Paired devices decrypt and update their clipboard automatically

## Installation

### Pre-built Binaries

Download the latest release for your platform:

| Platform | Download |
|----------|----------|
| Windows | [DecentPaste-x.x.x-windows.msi](https://github.com/decentpaste/decentpaste/releases) |
| macOS | [DecentPaste-x.x.x-macos.dmg](https://github.com/decentpaste/decentpaste/releases) |
| Linux (AppImage) | [DecentPaste-x.x.x-linux.AppImage](https://github.com/decentpaste/decentpaste/releases) |
| Linux (deb) | [DecentPaste-x.x.x-linux.deb](https://github.com/decentpaste/decentpaste/releases) |
| Android | [DecentPaste-x.x.x-android.apk](https://github.com/decentpaste/decentpaste/releases) |
| iOS | Coming soon |

### Build from Source

**Prerequisites:**
- [Rust](https://rustup.rs/) (1.70+)
- [Node.js](https://nodejs.org/) (18+)
- [Yarn](https://yarnpkg.com/)
- Platform-specific requirements for [Tauri](https://tauri.app/v2/guides/getting-started/prerequisites)

```bash
# Clone the repository
git clone https://github.com/decentpaste/decentpaste.git
cd decentpaste/decentpaste-app

# Install dependencies
yarn install

# Run in development mode
yarn tauri dev

# Build for production
yarn tauri build
```

## Usage

### Getting Started

1. **Install** DecentPaste on two or more devices
2. **Ensure** devices are on the same local network (Wi-Fi/LAN)
3. **Open** the app — devices will discover each other automatically
4. **Pair** devices using the 6-digit PIN
5. **Copy** on one device, **paste** on another!

### Pairing Devices

1. On Device A: Go to **Peers** → Click **Pair** next to the discovered device
2. On Device B: Accept the pairing request
3. Both devices display a **6-digit PIN** — verify they match
4. Confirm on Device A
5. Done! Devices are now paired and will sync automatically

### Mobile Usage

On Android and iOS:
- **Clipboard sharing**: Automatic monitoring is disabled. Use the **"Share Clipboard"** button to manually send content.
- **Pairing**: Keep the app open on both devices during pairing (background connections are not supported).

## Security

DecentPaste is designed with security as a priority:

| Layer | Technology | Purpose |
|-------|------------|---------|
| **Key Exchange** | X25519 ECDH | Secure key derivation without transmitting secrets |
| **Encryption** | AES-256-GCM | Authenticated encryption for clipboard content |
| **Hashing** | SHA-256 | Content deduplication and integrity verification |
| **Transport** | libp2p Noise | Encrypted peer-to-peer connections |

### How Pairing Security Works

1. Devices exchange **public keys** (X25519)
2. Each device independently derives the **same shared secret** using ECDH
3. The shared secret is **never transmitted** — only public keys are exchanged
4. 6-digit PIN provides **visual verification** against MITM attacks
5. Each device pair has a **unique encryption key**

### Data Storage

- **Shared secrets**: Stored locally in `~/.local/share/com.decentpaste.application/peers.json`
- **Private key**: Stored in `~/.local/share/com.decentpaste.application/private_key.bin`
- **No cloud**: All data stays on your devices

> **Note**: Secrets are currently stored in plaintext. OS keychain integration is planned for future releases.

## Tech Stack

| Component | Technology |
|-----------|------------|
| **App Framework** | [Tauri v2](https://tauri.app) |
| **Backend** | Rust |
| **Frontend** | TypeScript + [Tailwind CSS v4](https://tailwindcss.com) |
| **Networking** | [libp2p](https://libp2p.io) (mDNS, gossipsub, request-response) |
| **Encryption** | [aes-gcm](https://crates.io/crates/aes-gcm), [x25519-dalek](https://crates.io/crates/x25519-dalek) |

## Project Structure

```
decentpaste/
├── decentpaste-app/           # Main Tauri application
│   ├── src/                   # Frontend (TypeScript)
│   │   ├── app.ts             # Main UI
│   │   ├── api/               # Tauri command wrappers
│   │   └── state/             # Reactive state management
│   └── src-tauri/             # Backend (Rust)
│       ├── src/
│       │   ├── network/       # libp2p networking
│       │   ├── clipboard/     # Clipboard monitoring
│       │   ├── security/      # Encryption & pairing
│       │   └── storage/       # Settings & persistence
│       └── Cargo.toml
├── website/                   # Landing page
├── LICENSE                    # Apache 2.0
├── TRADEMARK.md               # Trademark policy
└── NOTICE                     # Third-party attributions
```

## Development

### Running Two Instances (Testing)

```bash
# Terminal 1
cd decentpaste-app
yarn tauri dev

# Terminal 2 (different port)
TAURI_DEV_PORT=1421 yarn tauri dev
```

### Architecture Documentation

See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed technical documentation.

## Roadmap

- [ ] Image and file clipboard support
- [ ] Internet relay for cross-network sync
- [ ] OS keychain integration for secret storage
- [x] Persistent clipboard history
- [ ] Browser extension
- [x] System tray improvements

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the **Apache License 2.0** — see the [LICENSE](LICENSE) file for details.

### Trademark

"DecentPaste" and the DecentPaste logo are trademarks. See [TRADEMARK.md](TRADEMARK.md) for usage guidelines.

**Note**: The Apache 2.0 license grants rights to the code, but does not grant rights to use the DecentPaste trademarks or logos.

## Acknowledgments

- [Tauri](https://tauri.app) — For the amazing cross-platform framework
- [libp2p](https://libp2p.io) — For decentralized networking
- [RustCrypto](https://github.com/RustCrypto) — For cryptographic primitives

---

<p align="center">
  <b>DecentPaste</b> — Your clipboard, everywhere.
  <br>
  <a href="https://github.com/decentpaste/decentpaste">GitHub</a> ·
  <a href="https://github.com/decentpaste/decentpaste/issues">Issues</a> ·
  <a href="https://github.com/decentpaste/decentpaste/releases">Releases</a>
</p>
