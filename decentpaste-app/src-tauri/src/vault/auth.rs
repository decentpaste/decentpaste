//! Vault authentication types for secure storage.
//!
//! This module defines the authentication state and method types used
//! throughout the vault system.

use serde::{Deserialize, Serialize};

/// Represents the current state of the vault.
///
/// The vault transitions between these states:
/// - `NotSetup` → `Unlocked` (after first-time setup with SecureStorage or PIN)
/// - `Unlocked` → `Locked` (when user locks or app backgrounds)
/// - `Locked` → `Unlocked` (after successful auth via biometric/keyring or PIN)
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VaultStatus {
    /// Vault has not been created yet (first-time user)
    #[default]
    NotSetup,
    /// Vault exists but is locked (requires authentication)
    Locked,
    /// Vault is open and data is accessible
    Unlocked,
}

/// Authentication method for vault encryption key.
///
/// Determines how the 256-bit vault encryption key is obtained:
/// - `SecureStorage`: Random key stored in platform secure storage (biometric/keyring)
/// - `Pin`: Key derived from user's PIN via Argon2id
/// - `SecureStorageWithPin` (desktop only): Random key encrypted with PIN, stored in keychain
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// Uses decentsecret plugin (biometric on mobile, keyring on desktop).
    /// Key is a random 256-bit value stored in platform secure storage.
    SecureStorage,
    /// PIN-based authentication with Argon2id key derivation.
    /// Key is derived from user's PIN + installation salt.
    Pin,
    /// Desktop-only: Encrypted vault key stored in OS keychain, requires PIN to decrypt.
    /// Provides 2-factor security: keychain access (what you have) + PIN (what you know).
    /// The vault key is encrypted with a PIN-derived key (Argon2id) before storage.
    #[cfg(desktop)]
    #[serde(rename = "secure_storage_with_pin")]
    SecureStorageWithPin,
}

impl std::fmt::Display for VaultStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSetup => write!(f, "NotSetup"),
            Self::Locked => write!(f, "Locked"),
            Self::Unlocked => write!(f, "Unlocked"),
        }
    }
}

impl std::fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SecureStorage => write!(f, "secure_storage"),
            Self::Pin => write!(f, "pin"),
            #[cfg(desktop)]
            Self::SecureStorageWithPin => write!(f, "secure_storage_with_pin"),
        }
    }
}
