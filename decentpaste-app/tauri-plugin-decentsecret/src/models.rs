//! Data types for the decentsecret plugin.

use serde::{Deserialize, Serialize};

/// The method used for secure secret storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretStorageMethod {
    /// Android biometric authentication with TEE/StrongBox.
    AndroidBiometric,
    /// iOS biometric authentication with Secure Enclave.
    IOSBiometric,
    /// macOS Keychain.
    MacOSKeychain,
    /// Windows Credential Manager.
    WindowsCredentialManager,
    /// Linux Secret Service API (GNOME Keyring, KWallet, etc.)
    LinuxSecretService,
}

/// Status of secure secret storage availability.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStorageStatus {
    /// Whether secure storage is available and can be used.
    pub available: bool,
    /// The method that will be used (if available).
    pub method: Option<SecretStorageMethod>,
    /// Why secure storage is unavailable (if not available).
    pub unavailable_reason: Option<String>,
}

impl SecretStorageStatus {
    /// Create a status indicating secure storage is available.
    pub fn available(method: SecretStorageMethod) -> Self {
        Self {
            available: true,
            method: Some(method),
            unavailable_reason: None,
        }
    }

    /// Create a status indicating secure storage is unavailable.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            available: false,
            method: None,
            unavailable_reason: Some(reason.into()),
        }
    }
}

/// Request to store a secret.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreSecretRequest {
    /// The secret bytes to store (typically a 32-byte vault key).
    pub secret: Vec<u8>,
}

/// Response from retrieving a secret.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrieveSecretResponse {
    /// The retrieved secret bytes.
    pub secret: Vec<u8>,
}

/// Empty response for store/delete operations.
/// Mobile plugins return {} which needs to deserialize into a struct, not ().
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EmptyResponse {}
