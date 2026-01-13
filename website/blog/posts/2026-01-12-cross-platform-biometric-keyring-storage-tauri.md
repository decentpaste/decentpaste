---
title: How to Implement Cross-Platform Biometric/Keyring Secret Storage in Tauri v2
date: 2026-01-12
description: A deep dive into building secure secret storage across iOS, Android, macOS, Windows, and Linux - including the gotchas that took hours to debug.
---

After my [previous post about iOS Share Extensions](https://www.reddit.com/r/tauri/comments/1q5s1ec/how_to_implement_ios_share_extension_plugin_in/), I wanted to share another Tauri v2 plugin I built: cross-platform secure secret storage. If you need to store encryption keys, API tokens, or any sensitive data that should *never* touch plain storage, this might help.

## Why I Needed This

I'm building [DecentPaste](https://github.com/decentpaste/decentpaste) - a P2P clipboard sharing app. The app encrypts everything with AES-256-GCM, which means I need to store a 256-bit encryption key somewhere safe.

The requirements:
- Key cannot live in localStorage, files, or any unencrypted storage
- On mobile, users expect biometric protection (Face ID / fingerprint)
- On desktop, it should use the OS keyring transparently

Sounds simple. Then I looked at what "secure storage" actually means on each platform.

## The Gotchas That Hurt

### 1. Biometric Changes = Your Key Is Gone. Forever.

This one cost me hours of debugging before I understood it was *intentional*.

If a user adds a new fingerprint, removes Face ID, or changes their biometric enrollment in any way, all biometric-protected keys become permanently inaccessible. The hardware invalidates them.

This is a security feature. If someone steals your phone and adds their fingerprint, they shouldn't get access to your old secrets.

But your app needs to handle this gracefully:

```
Error: BIOMETRIC_CHANGED - Key invalidated due to enrollment change
```

When you see this, you have to reset your vault and re-setup. There's no recovery path. Here's my [error handling approach](https://github.com/decentpaste/decentpaste/blob/main/decentpaste-app/tauri-plugin-decentsecret/src/error.rs).

### 2. Async Biometric Prompts vs Sync Tauri Commands

Android's `BiometricPrompt` is callback-based. You show the prompt, then get results in `onAuthenticationSucceeded()` or `onAuthenticationFailed()`.

Tauri commands expect you to return a result directly.

The solution: store pending command invocations in a map, then resolve them when the callback fires.

```kotlin
private val pendingInvokes = ConcurrentHashMap<String, Invoke>()

// When command comes in:
val invokeId = System.currentTimeMillis().toString()
pendingInvokes[invokeId] = invoke
showBiometricPrompt(invokeId, ...)

// Later, in callback:
override fun onAuthenticationSucceeded(...) {
    val invoke = pendingInvokes.remove(invokeId) ?: return
    invoke.resolve(result)
}
```

Not elegant, but it works reliably. Full [Kotlin implementation here](https://github.com/decentpaste/decentpaste/blob/main/decentpaste-app/tauri-plugin-decentsecret/android/src/main/java/DecentsecretPlugin.kt).

### 3. iOS Error Codes Are Cryptic

Android throws `KeyPermanentlyInvalidatedException`. Clear, descriptive, you know exactly what happened.

iOS gives you error code `-25293`. Good luck googling that.

I had to resort to parsing error message strings:

```swift
if errorMsg.contains("invalidat") || errorMsg.contains("LAError") {
    invoke.reject("BIOMETRIC_CHANGED: \(errorMsg)")
}
```

It's hacky, but it's the only reliable way I found to detect biometric enrollment changes on iOS. See the [Swift implementation](https://github.com/decentpaste/decentpaste/blob/main/decentpaste-app/tauri-plugin-decentsecret/ios/Sources/DecentsecretPlugin.swift).

### 4. Linux Keyring: Per-Session vs Per-Login

The `keyring` crate has a `linux-native` feature that uses the kernel keyring. Problem: it stores secrets per-session, not per-login. Reboot your machine and your secrets are gone.

Instead, use `sync-secret-service` which talks to GNOME Keyring / KWallet via D-Bus (gdbus). These actually persist across reboots.

Also, the default keyring features pull in OpenSSL. If you want to avoid that dependency hell on Linux, use `crypto-rust`:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
keyring = { version = "3", default-features = false, features = ["sync-secret-service", "crypto-rust"] }
```

### 5. Zeroize Your Secrets

When you're done with a key in memory, zero it out. Don't just let it go out of scope and hope the allocator overwrites it eventually.

The `zeroize` crate handles this - it zeros memory on drop and prevents the compiler from optimizing away the zeroing. For encryption keys that live in RAM, this matters.

## Bonus: 2FA Mode for Desktop

Keyring storage alone isn't always enough. If someone has access to your OS session, they can read your secrets. On mobile, biometrics verify presence every time. On desktop, there's no such check.

So I added a 2FA mode: Keyring + PIN.

The vault key gets encrypted with an Argon2id-derived key from the user's PIN, then stored in the keyring. To unlock, you need both:
1. OS session access (to read the keyring)
2. The PIN (to decrypt the vault key)

Beyond security, there's also a UX angle: some users just *want* to lock their vault. Having a PIN gives them that control - they can step away from their computer knowing their clipboard history isn't accessible. It's psychological, but it matters. The [vault manager](https://github.com/decentpaste/decentpaste/blob/main/decentpaste-app/src-tauri/src/vault/manager.rs) handles both modes.

## Quick Checklist

If you're building something similar:

- Handle biometric enrollment changes explicitly - don't let users hit a wall
- Use `zeroize` for any secrets in memory
- On Linux, use `sync-secret-service` not `linux-native` if you want persistence
- Test on real devices - emulators fake biometric APIs
- Consider 2FA for desktop if your threat model needs it

## Don't Do This

- Store secrets in SharedPreferences or UserDefaults directly
- Ignore the "key invalidated" error - your users will be locked out
- Pull in OpenSSL on Linux when `crypto-rust` works fine
- Let keys linger in memory after use

## Lessons Learned

This was my second Tauri plugin after the [iOS Share Extension](https://www.reddit.com/r/tauri/comments/1q5s1ec/how_to_implement_ios_share_extension_plugin_in/). The code itself isn't that complex - most of it is just calling platform APIs correctly.

The hard part was the research: figuring out that `linux-native` doesn't persist, that iOS error codes are meaningless, that biometric invalidation is intentional.

If you want to build your own plugin, Tauri makes the scaffolding easy:

```bash
npx @tauri-apps/cli plugin new --android --ios <plugin-name>
```

This generates the full structure with Rust, Kotlin, and Swift boilerplate ready to go.

## Links

- [Full plugin implementation](https://github.com/decentpaste/decentpaste/tree/main/decentpaste-app/tauri-plugin-decentsecret)
- [Tauri v2 Plugin Development Guide](https://v2.tauri.app/develop/plugins/) - start here if you're new to Tauri plugins
- [DecentPaste](https://github.com/decentpaste/decentpaste)
- [The website](https://decentpaste.com/)

Happy to answer questions if you're working on something similar!
