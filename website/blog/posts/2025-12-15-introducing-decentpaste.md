---
title: Introducing DecentPaste
date: 2025-12-15
description: Today we're excited to publicly launch DecentPaste - a privacy-first clipboard sharing app that works across all your devices without cloud servers.
---

Today marks an exciting milestone: the public launch of DecentPaste.

## Why We Built This

In a world where cloud services handle everything, we believed there was still room for a simpler, more private approach to clipboard sharing. Apple's Universal Clipboard is great, but it only works within the Apple ecosystem. Windows and Android users are left searching for alternatives that often require accounts, subscriptions, or cloud storage.

**DecentPaste is different.** It uses peer-to-peer networking over your local network - your clipboard data never leaves your home or office. No accounts, no cloud servers, no subscriptions.

## Key Features

Here's what makes DecentPaste special:

- **End-to-end encryption** - Your data is encrypted with AES-256-GCM before it leaves your device
- **No cloud servers** - Everything happens on your local network using libp2p
- **Cross-platform** - Works on Windows, Mac, Linux, Android, and iOS
- **Open source** - Fully auditable under the Apache-2.0 license

## How It Works

The pairing process uses X25519 elliptic curve Diffie-Hellman for key exchange. When you pair two devices:

1. Both devices generate ephemeral key pairs
2. They exchange public keys over the local network
3. Each device derives the same shared secret mathematically
4. This shared secret is used to encrypt all future clipboard data

> The beauty of this approach is that the shared secret is never transmitted - it's computed independently on both devices.

Here's a simplified example of how the key derivation works in Rust:

```rust
use x25519_dalek::{EphemeralSecret, PublicKey};

fn derive_shared_secret() {
    // Generate ephemeral key pairs on each device
    let alice_secret = EphemeralSecret::random();
    let alice_public = PublicKey::from(&alice_secret);

    let bob_secret = EphemeralSecret::random();
    let bob_public = PublicKey::from(&bob_secret);

    // Each device computes the same shared secret
    let alice_shared = alice_secret.diffie_hellman(&bob_public);
    let bob_shared = bob_secret.diffie_hellman(&alice_public);

    // alice_shared == bob_shared (mathematically guaranteed)
}
```

And on the TypeScript side, invoking the Rust backend is simple:

```typescript
import { invoke } from '@tauri-apps/api/core';

async function pairDevice(peerCode: string) {
  try {
    await invoke('pair_device', { code: peerCode });
    console.log('Device paired successfully!');
  } catch (error) {
    console.error('Pairing failed:', error);
  }
}
```

## Getting Started

Getting started is simple:

1. Download DecentPaste on your devices
2. Set a PIN to protect your encrypted vault
3. Pair your devices using the 6-digit code
4. Start copying!

That's it. No account creation, no email verification, no cloud setup.

## What's Next

We have big plans for DecentPaste:

- **Image and file support** - Currently text-only, but we're working on binary data
- **Internet relay** - For clipboard sync when devices aren't on the same network
- **Browser extension** - One-click paste from your desktop to any website

## Open Source

DecentPaste is fully open source. You can audit the code, contribute improvements, or fork it for your own use:

[View on GitHub](https://github.com/decentpaste/decentpaste)

We believe privacy software should be transparent. If you find any issues or have suggestions, please open an issue on GitHub.

---

Thanks for checking out DecentPaste. We hope it makes your cross-device workflow a little bit easier - and a lot more private.
