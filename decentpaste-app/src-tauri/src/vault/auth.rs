//! Vault authentication types for secure storage.
//!
//! This module defines the authentication state and method types used
//! throughout the vault system.

use serde::{Deserialize, Serialize};

/// Represents the current state of the vault.
///
/// The vault transitions between these states:
/// - `NotSetup` → `Unlocked` (after first-time setup with PIN)
/// - `Unlocked` → `Locked` (when user locks or app backgrounds)
/// - `Locked` → `Unlocked` (after successful PIN/biometric auth)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VaultStatus {
    /// Vault has not been created yet (first-time user)
    NotSetup,
    /// Vault exists but is locked (requires authentication)
    Locked,
    /// Vault is open and data is accessible
    Unlocked,
}

impl Default for VaultStatus {
    fn default() -> Self {
        Self::NotSetup
    }
}

/// Authentication method preference for unlocking the vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMethod {
    /// PIN-based authentication (4-8 digits)
    Pin,
    /// Biometric authentication (fingerprint, face, etc.)
    /// Falls back to PIN if biometric fails or is unavailable
    Biometric,
}

impl Default for AuthMethod {
    fn default() -> Self {
        Self::Pin
    }
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
            Self::Pin => write!(f, "pin"),
            Self::Biometric => write!(f, "biometric"),
        }
    }
}
