# Biometric Authentication Implementation Plan

**Status**: Revised after security review
**Priority**: High
**Target**: Mobile (Android/iOS) with hardware biometrics, Desktop with OS keyring

---

## Security Review Summary

> **Review Date**: 2025-01-10
> **Reviewer**: Claude Code Security Analysis
> **Verdict**: Original plan had critical flaws; revised plan addresses all issues

### Critical Issues Identified & Resolved

| Issue | Original Plan | Resolution |
|-------|--------------|------------|
| Wrapped key inside vault | Stored inside Stronghold (impossible) | Store in platform secure storage |
| Memory zeroing | Used `drop()` (doesn't zero) | Use `zeroize` crate |
| Android thread safety | Single `pendingInvoke` variable | Use `ConcurrentHashMap` |
| Stronghold complexity | 20+ deps, no security benefit | **Remove entirely** |
| PIN fallback | Created dual-key weakness | PIN/biometric mutually exclusive |
| iOS key invalidation | Used `.userPresence` | Use `.biometryCurrentSet` |
| iOS encryption | Unclear approach | Use Secure Enclave + ECIES |

### User Decisions (Confirmed)

1. **Wrapped Key Storage**: Platform secure storage (AndroidKeyStore / iOS Keychain), NOT inside vault
2. **Key Invalidation**: Invalidate on biometric enrollment change (more secure)
3. **PIN/Biometric Relationship**: Mutually exclusive - device capability determines which, not fallback
4. **iOS Algorithm**: ECIES with Secure Enclave for maximum hardware security
5. **Stronghold**: Remove entirely - no security benefit over existing AES-256-GCM + Argon2id
6. **Desktop**: Use `keyring` crate for session-based OS keychain security

---

## Overview

This plan describes the implementation of hardware-backed authentication for DecentPaste's vault system.

### Problem Statement

The current PIN-based vault authentication has security limitations:
- PIN is brute-forceable (though Argon2id slows it significantly)
- PIN must be derived each time (memory-intensive)
- No hardware protection (relies on KDF strength)
- Stronghold adds complexity without additional security benefit

### Solution

Implement platform-appropriate authentication:

| Platform | Auth Method | Storage | Security Level |
|----------|-------------|---------|----------------|
| **Android** | Hardware biometric (TEE) | AndroidKeyStore | Strongest |
| **iOS** | Hardware biometric (Secure Enclave) | iOS Keychain | Strongest |
| **macOS** | OS session (user login) | keyring → macOS Keychain | Good |
| **Windows** | OS session (user login) | keyring → Credential Manager | Good |
| **Linux** | OS session (user login) | keyring → Secret Service | Good |
| **Fallback** | PIN (Argon2id) | Encrypted file | Good |

---

## Revised Architecture

### Major Change: Remove Stronghold

**Why Stronghold is being removed:**
1. No security benefit - DecentPaste already has AES-256-GCM + Argon2id
2. Work factor disabled to 0 (35-second delays were unacceptable)
3. Features unused - client isolation, secret rotation, ACLs
4. 20+ transitive dependencies for a simple encrypted JSON store
5. Creates awkward constraints for biometric key storage

**Replacement:** Simple encrypted file using existing crypto primitives.

```rust
#[derive(Serialize, Deserialize)]
pub struct VaultData {
    pub clipboard_history: Vec<ClipboardEntry>,
    pub paired_peers: Vec<PairedPeer>,
    pub device_identity: Option<DeviceIdentity>,
    pub libp2p_keypair: Option<Vec<u8>>,
}

// Encryption: AES-256-GCM (already exists in security/crypto.rs)
// Key derivation: Argon2id for PIN, random for biometric/keyring
```

### Security Model

```
MOBILE (Android/iOS) - Hardware-Bound:
──────────────────────────────────────
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│ User opens  │ ──► │ Biometric prompt │ ──► │ Hardware    │
│ app         │     │ (Fingerprint/    │     │ releases    │
│             │     │  Face ID)        │     │ vault key   │
└─────────────┘     └──────────────────┘     └─────────────┘
                           │
                           ▼
                    Hardware-bound key
                    (Cannot be accessed without biometric)

DESKTOP (macOS/Windows/Linux) - Session-Bound:
──────────────────────────────────────────────
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│ User logs   │ ──► │ OS keychain      │ ──► │ App reads   │
│ into OS     │     │ unlocked         │     │ vault key   │
│             │     │                  │     │ from keyring│
└─────────────┘     └──────────────────┘     └─────────────┘
                           │
                           ▼
                    Session-bound key
                    (Accessible while user session active)
```

### Data Flow: Biometric Vault Setup

```
┌─────────────────────────────────────────────────────────────────┐
│                  BIOMETRIC VAULT SETUP (Mobile)                  │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Generate random 256-bit vault key                           │
│     vault_key = rand::random::<[u8; 32]>()                      │
│                                                                  │
│  2. Wrap vault key with platform biometrics                     │
│     ┌─────────────────────────────────────────────────────┐     │
│     │ Android: AndroidKeyStore + AES-GCM                  │     │
│     │   - Generate AES key in TEE                         │     │
│     │   - setUserAuthenticationRequired(true)             │     │
│     │   - setInvalidatedByBiometricEnrollment(true)       │     │
│     │   - Encrypt vault_key → wrapped_key + iv            │     │
│     ├─────────────────────────────────────────────────────┤     │
│     │ iOS: Secure Enclave + ECIES                         │     │
│     │   - Generate EC key in Secure Enclave               │     │
│     │   - kSecAttrTokenIDSecureEnclave                    │     │
│     │   - .biometryCurrentSet access control              │     │
│     │   - ECIES encrypt vault_key → wrapped_key           │     │
│     └─────────────────────────────────────────────────────┘     │
│                                                                  │
│  3. Store wrapped key in platform secure storage                │
│     - Android: SharedPreferences (encrypted key in KeyStore)   │
│     - iOS: Keychain (encrypted data)                           │
│                                                                  │
│  4. Create vault file with vault key                            │
│     - Serialize VaultData to JSON                               │
│     - Encrypt with AES-256-GCM using vault_key                  │
│     - Write to vault.enc                                        │
│                                                                  │
│  5. Zeroize vault key from memory                               │
│     vault_key.zeroize()  // Uses zeroize crate                  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow: Desktop Keyring Setup

```
┌─────────────────────────────────────────────────────────────────┐
│                  KEYRING VAULT SETUP (Desktop)                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Generate random 256-bit vault key                           │
│     vault_key = rand::random::<[u8; 32]>()                      │
│                                                                  │
│  2. Store vault key in OS keychain via keyring crate            │
│     - macOS: Keychain Access                                    │
│     - Windows: Credential Manager                               │
│     - Linux: Secret Service (GNOME Keyring, KWallet)           │
│                                                                  │
│  3. Create vault file with vault key                            │
│     - Same as mobile: JSON → AES-256-GCM → vault.enc            │
│                                                                  │
│  4. Zeroize vault key from memory                               │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Security Properties Comparison

| Property | PIN Vault | Biometric Vault | Keyring Vault |
|----------|-----------|-----------------|---------------|
| **Key Type** | Derived (Argon2id) | Random 256-bit | Random 256-bit |
| **Key Storage** | Not stored | Hardware (TEE/SE) | OS keychain |
| **Brute-force** | ~10 years | Infeasible (2^256) | Infeasible (2^256) |
| **Stolen device** | Requires PIN | Requires biometrics | Requires OS login |
| **Biometric change** | N/A | Key invalidated | N/A |
| **Memory exposure** | Brief (derivation) | Brief (unlock) | Brief (unlock) |
| **Hardware binding** | No | Yes | No (session-bound) |

---

## Implementation Phases

### Phase 0: Remove Stronghold & Add Zeroize
**Goal**: Replace Stronghold with simple AES-256-GCM encrypted file

**Tasks**:
- Add `zeroize = "1.7"` to Cargo.toml
- Remove `tauri-plugin-stronghold` and `iota_stronghold` dependencies
- Create `vault/storage.rs` with `VaultData` struct
- Create `ZeroingVaultKey` wrapper with secure memory clearing
- Implement `save_vault()` / `load_vault()` using existing crypto
- Update `VaultManager` to use new storage
- Remove Stronghold plugin init from `lib.rs`

**Files**:
- `decentpaste-app/src-tauri/Cargo.toml`
- `decentpaste-app/src-tauri/src/vault/storage.rs` (NEW)
- `decentpaste-app/src-tauri/src/vault/manager.rs`
- `decentpaste-app/src-tauri/src/lib.rs`

### Phase 1: Desktop Keyring Integration
**Goal**: Store vault key in OS keychain for session-based security

**Tasks**:
- Add `keyring = "3"` to Cargo.toml (desktop only)
- Create `vault/keyring_storage.rs` with keyring wrapper
- Add `AuthMethod::Keyring` variant
- Implement `create_with_keyring()` and `open_with_keyring()`
- Desktop auto-unlocks via OS keychain

**Files**:
- `decentpaste-app/src-tauri/Cargo.toml`
- `decentpaste-app/src-tauri/src/vault/keyring_storage.rs` (NEW)
- `decentpaste-app/src-tauri/src/vault/auth.rs`
- `decentpaste-app/src-tauri/src/vault/manager.rs`

### Phase 2: Mobile Biometric Plugin
**Goal**: Create `tauri-plugin-decentsecret` for hardware-bound biometric auth

**Critical Security Requirements**:
- Android: Use `ConcurrentHashMap` for pending operations (NOT single variable)
- Android: Set `setInvalidatedByBiometricEnrollment(true)`
- Android: Use `CryptoObject` pattern for hardware binding
- iOS: Use `.biometryCurrentSet` (NOT `.userPresence`)
- iOS: Use `kSecAttrTokenIDSecureEnclave` for Secure Enclave

**API**:
```rust
// Unified Rust API
pub fn check_biometric_status() -> BiometricStatus;
pub fn wrap_vault_key(key: Vec<u8>) -> WrapKeyResult;
pub fn unwrap_vault_key() -> UnwrapKeyResult;
pub fn delete_vault_key() -> DeleteKeyResult;
```

**Files**:
- `tauri-plugin-decentsecret/` (entire plugin directory)
- Android: `DecentsecretPlugin.kt`
- iOS: `DecentsecretPlugin.swift`

### Phase 3: VaultManager Mobile Integration
**Goal**: Wire biometric plugin into vault system

**Tasks**:
- Add `AuthMethod::Biometric` variant
- Implement `create_with_biometric()` and `open_with_biometric()`
- Register plugin in `lib.rs` (mobile only)
- Zeroize keys after use

### Phase 4: Commands & Frontend Integration
**Goal**: Unified API and UI for all auth methods

**Tasks**:
- Add `check_biometric_available()` command
- Add `setup_vault_auto()` / `unlock_vault_auto()` commands
- Update frontend to auto-detect auth method
- Mobile: show biometric prompt
- Desktop: auto-unlock via keyring

### Phase 5: Testing & Documentation
**Goal**: Validate security and update docs

**Tests**:
- Biometric enrollment change invalidates keys
- Desktop keyring persistence across app restarts
- PIN fallback works when biometric/keyring unavailable
- Cross-device clipboard sync with new vault format

---

## Android Implementation Details

### BiometricPrompt with CryptoObject

```kotlin
// CRITICAL: Use ConcurrentHashMap for thread safety
private val pendingOperations = ConcurrentHashMap<String, PendingOperation>()

@Command
fun wrapVaultKey(invoke: Invoke, key: ByteArray) {
    val operationId = UUID.randomUUID().toString()
    pendingOperations[operationId] = PendingOperation(invoke, OperationType.WRAP, key)

    val secretKey = getOrCreateBiometricKey()
    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
    cipher.init(Cipher.ENCRYPT_MODE, secretKey)

    showBiometricPrompt(operationId, cipher, "Secure your vault")
}

private fun getOrCreateBiometricKey(): SecretKey {
    val builder = KeyGenParameterSpec.Builder(KEY_ALIAS, ...)
        .setUserAuthenticationRequired(true)
        .setInvalidatedByBiometricEnrollment(true)  // SECURITY: Invalidate on change
    // ...
}
```

### Key Generation Parameters

```kotlin
KeyGenParameterSpec.Builder(KEY_ALIAS, PURPOSE_ENCRYPT or PURPOSE_DECRYPT)
    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
    .setKeySize(256)
    .setUserAuthenticationRequired(true)
    .setInvalidatedByBiometricEnrollment(true)  // CRITICAL
    .setUserAuthenticationParameters(0, AUTH_BIOMETRIC_STRONG)  // Require each time
```

---

## iOS Implementation Details

### Secure Enclave + ECIES

```swift
// CRITICAL: Use .biometryCurrentSet (NOT .userPresence)
guard let accessControl = SecAccessControlCreateWithFlags(
    nil,
    kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
    [.privateKeyUsage, .biometryCurrentSet],  // SECURITY: Invalidate on change
    nil
) else { ... }

// CRITICAL: Use Secure Enclave
let keyParams: [String: Any] = [
    kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
    kSecAttrKeySizeInBits as String: 256,
    kSecAttrTokenID as String: kSecAttrTokenIDSecureEnclave,  // SECURITY: Hardware
    kSecPrivateKeyAttrs as String: [
        kSecAttrIsPermanent as String: true,
        kSecAttrApplicationTag as String: keyTag,
        kSecAttrAccessControl as String: accessControl
    ]
]
```

### ECIES Encryption

```swift
// Encrypt vault key with Secure Enclave public key
let encryptedData = SecKeyCreateEncryptedData(
    publicKey,
    .eciesEncryptionCofactorVariableIVX963SHA256AESGCM,
    vaultKey as CFData,
    &error
)
```

---

## Desktop Keyring Implementation

### Using keyring crate

```rust
use keyring::Entry;

const SERVICE_NAME: &str = "com.decentpaste.vault";
const KEY_NAME: &str = "vault_key";

pub struct KeyringStorage {
    entry: Entry,
}

impl KeyringStorage {
    pub fn store_vault_key(&self, key: &[u8; 32]) -> Result<()> {
        let encoded = base64::encode(key);
        self.entry.set_password(&encoded)?;
        Ok(())
    }

    pub fn retrieve_vault_key(&self) -> Result<[u8; 32]> {
        let encoded = self.entry.get_password()?;
        let decoded = base64::decode(&encoded)?;
        Ok(decoded.try_into()?)
    }
}
```

---

## Future Extensibility

The `tauri-plugin-decentsecret` API is designed for future desktop biometric support:

```
Phase 2 (Current):  Android + iOS biometric
Phase 6 (Future):   macOS TouchID via plugin
Phase 7 (Future):   Windows Hello via plugin
```

When implementing desktop biometrics later:
- Add `macos` and `windows` implementations to the same plugin
- macOS: `LocalAuthentication` framework + Keychain access control
- Windows: `Windows.Security.Credentials.UI` + DPAPI
- The Rust API remains identical - frontend code requires zero changes

---

## Dependencies

### New Dependencies

**App Cargo.toml**:
```toml
# Memory safety
zeroize = { version = "1.7", features = ["derive"] }

# Desktop keyring (not for mobile)
[target.'cfg(not(any(target_os = "android", target_os = "ios")))'.dependencies]
keyring = "3"

# Plugin
tauri-plugin-decentsecret = { path = "../tauri-plugin-decentsecret" }
```

### Removed Dependencies

```toml
# REMOVED - no longer needed
# tauri-plugin-stronghold = "2"
# iota_stronghold = "2.1"
```

---

## Success Criteria

- [ ] Stronghold completely removed from codebase
- [ ] `zeroize` crate properly clears vault keys from memory
- [ ] Desktop: Vault auto-unlocks via OS keychain
- [ ] Android: Biometric auth with hardware binding works
- [ ] iOS: Biometric auth with Secure Enclave works
- [ ] Biometric enrollment change invalidates keys (mobile)
- [ ] PIN fallback works for devices without biometrics
- [ ] Cross-platform clipboard sync works with new vault format
- [ ] Documentation updated (SECURITY.md, ARCHITECTURE.md)

---

## References

### Libraries
- [keyring crate](https://crates.io/crates/keyring) - Cross-platform credential storage
- [zeroize crate](https://crates.io/crates/zeroize) - Secure memory zeroing

### Platform APIs
- **Android**: [BiometricPrompt](https://developer.android.com/training/sign-in/biometric-auth), [AndroidKeyStore](https://developer.android.com/training/articles/keystore)
- **iOS**: [LocalAuthentication](https://developer.apple.com/documentation/localauthentication), [Secure Enclave](https://developer.apple.com/documentation/security/certificate_key_and_trust_services/keys/protecting_keys_with_the_secure_enclave)

### Security Guidelines
- [OWASP Mobile Security](https://owasp.org/www-project-mobile-security/)
- [Android Biometric Best Practices](https://developer.android.com/training/sign-in/biometric-auth#best-practices)
- [Apple Secure Enclave Documentation](https://support.apple.com/guide/security/secure-enclave-sec59b0b31ff/web)
