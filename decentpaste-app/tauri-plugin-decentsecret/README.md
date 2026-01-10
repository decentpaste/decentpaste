# Tauri Plugin: decentsecret

Hardware-backed secure secret storage for Tauri v2 applications.

This plugin provides a unified API for storing secrets using platform-native security mechanisms:

| Platform | Backend                           | Security Level  |
|----------|-----------------------------------|-----------------|
| Android  | AndroidKeyStore + BiometricPrompt | TEE/StrongBox   |
| iOS      | Keychain + Secure Enclave         | Hardware-backed |
| macOS    | Keychain Access                   | Session-based   |
| Windows  | Credential Manager                | Session-based   |
| Linux    | Secret Service API                | Session-based   |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
tauri-plugin-decentsecret = { path = "../tauri-plugin-decentsecret" }
```

Register in your Tauri app:

```rust
// src-tauri/src/lib.rs
tauri::Builder::default()
    .plugin(tauri_plugin_decentsecret::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

Add to capabilities:

```json
// src-tauri/capabilities/default.json
{
  "permissions": ["decentsecret:default"]
}
```

## TypeScript API

```typescript
import {
  checkAvailability,
  storeSecret,
  retrieveSecret,
  deleteSecret,
  SecretStorageStatus,
} from 'tauri-plugin-decentsecret-api';

// Check if hardware security is available
const status: SecretStorageStatus = await checkAvailability();
if (status.available) {
  console.log(`Using: ${status.method}`);
} else {
  console.log(`Unavailable: ${status.unavailableReason}`);
}

// Store a 32-byte key
const key = new Uint8Array(32);
crypto.getRandomValues(key);
await storeSecret(Array.from(key));

// Retrieve the key (triggers biometric on mobile)
const retrieved = await retrieveSecret();
const vaultKey = new Uint8Array(retrieved);

// Delete the key (vault reset)
await deleteSecret();
```

## Rust API

```rust
use tauri_plugin_decentsecret::{DecentsecretExt, SecretStorageStatus};

// Check availability
let status: SecretStorageStatus = app_handle.decentsecret().check_availability().await?;

// Store secret
app_handle.decentsecret().store_secret(key_bytes).await?;

// Retrieve secret (triggers biometric on mobile)
let key: Vec<u8> = app_handle.decentsecret().retrieve_secret().await?;

// Delete secret
app_handle.decentsecret().delete_secret().await?;
```

## Platform Behavior

### Mobile (Android/iOS)

- **Biometric prompt**: Every `storeSecret` and `retrieveSecret` call shows a biometric prompt
- **TEE/Secure Enclave**: Keys are stored in hardware and never leave
- **Biometric binding**: If the user adds/removes fingerprints or changes Face ID, stored secrets become **permanently inaccessible** (`BiometricEnrollmentChanged` error)

### Desktop (macOS/Windows/Linux)

- **Session-based**: No per-operation prompt; secrets are accessible after OS login
- **OS keyring**: Uses the platform's native credential storage
- **Linux requirement**: Requires a running Secret Service (GNOME Keyring or KWallet)

## Error Handling

```typescript
try {
  const secret = await retrieveSecret();
} catch (error) {
  const msg = error.message || error;

  if (msg.includes('BiometricEnrollmentChanged')) {
    // User's biometrics changed - vault is permanently inaccessible
    // Must reset vault and re-setup
  } else if (msg.includes('UserCancelled')) {
    // User dismissed the biometric prompt
  } else if (msg.includes('SecretNotFound')) {
    // No secret stored yet
  } else if (msg.includes('AuthenticationFailed')) {
    // Biometric didn't match
  }
}
```

## Security Considerations

1. **Biometric change = data loss** (mobile): This is intentional security behavior. If biometrics change, an attacker may have enrolled their own fingerprint.

2. **No secret recovery**: There is no way to recover a secret if biometrics change. Design your app to handle vault reset gracefully.

3. **Desktop is session-based**: macOS Keychain, Windows Credential Manager, and Linux Secret Service don't require per-access authentication. The secret is accessible while the user is logged in.

4. **Single secret per app**: This plugin stores one secret identified by the app's bundle ID. For multiple secrets, encrypt them with the stored key.

## Android Setup

Add biometric permission to `AndroidManifest.xml`:

```xml
<uses-permission android:name="android.permission.USE_BIOMETRIC" />
```

The plugin requires `minSdkVersion 23` (Android 6.0+).

## iOS Setup

Add Face ID usage description to `Info.plist`:

```xml
<key>NSFaceIDUsageDescription</key>
<string>Unlock your vault with Face ID</string>
```

## License

Apache-2.0
