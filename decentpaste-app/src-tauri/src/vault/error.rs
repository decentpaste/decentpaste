//! Vault-specific error types for secure storage operations.
//!
//! This module provides granular error handling for vault operations,
//! allowing the frontend to display appropriate user-facing messages
//! and take corrective actions.

use thiserror::Error;

/// Errors that can occur during vault operations.
///
/// These errors are designed to be:
/// - Specific enough for programmatic handling
/// - User-friendly for display in the UI
/// - Convertible to the main application error type
#[derive(Error, Debug)]
pub enum VaultError {
    /// The provided PIN is incorrect.
    /// User should try again or use the reset option.
    #[error("Invalid PIN")]
    InvalidPin,

    /// The vault has not been set up yet.
    /// User needs to complete onboarding first.
    #[error("Vault not set up")]
    NotSetup,

    /// The vault is locked and requires authentication.
    /// User needs to enter PIN.
    #[error("Vault is locked")]
    Locked,

    /// The vault file exists but appears to be corrupted.
    /// User may need to reset the vault.
    #[error("Vault data is corrupted: {0}")]
    Corrupted(String),

    /// An error occurred in the encryption layer.
    #[error("Encryption error: {0}")]
    Encryption(String),

    /// An I/O error occurred (file access, permissions, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization/deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The salt file is missing or invalid.
    #[error("Salt error: {0}")]
    Salt(String),

    /// Key derivation failed (Argon2 error).
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),

    /// The vault file already exists when trying to create.
    #[error("Vault already exists")]
    AlreadyExists,
}

/// Result type alias for vault operations.
pub type VaultResult<T> = std::result::Result<T, VaultError>;

// ============================================================================
// Conversions to main application error type
// ============================================================================

impl From<VaultError> for crate::error::DecentPasteError {
    fn from(err: VaultError) -> Self {
        match err {
            VaultError::InvalidPin => crate::error::DecentPasteError::InvalidPin,
            VaultError::NotSetup => {
                crate::error::DecentPasteError::Storage("Vault not set up".into())
            }
            VaultError::Locked => crate::error::DecentPasteError::Storage("Vault is locked".into()),
            VaultError::Corrupted(msg) => {
                crate::error::DecentPasteError::Storage(format!("Vault corrupted: {}", msg))
            }
            VaultError::Encryption(msg) => {
                crate::error::DecentPasteError::Encryption(format!("Vault encryption: {}", msg))
            }
            VaultError::Io(e) => crate::error::DecentPasteError::Io(e),
            VaultError::Serialization(e) => crate::error::DecentPasteError::Serialization(e),
            VaultError::Salt(msg) => {
                crate::error::DecentPasteError::Storage(format!("Salt error: {}", msg))
            }
            VaultError::KeyDerivation(msg) => {
                crate::error::DecentPasteError::Encryption(format!("Key derivation: {}", msg))
            }
            VaultError::AlreadyExists => {
                crate::error::DecentPasteError::Storage("Vault already exists".into())
            }
        }
    }
}

// ============================================================================
// Serialization for Tauri IPC
// ============================================================================

impl serde::Serialize for VaultError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        // Serialize as a structured object for better frontend handling
        let mut state = serializer.serialize_struct("VaultError", 2)?;

        // Error code for programmatic handling
        let code = match self {
            VaultError::InvalidPin => "INVALID_PIN",
            VaultError::NotSetup => "NOT_SETUP",
            VaultError::Locked => "LOCKED",
            VaultError::Corrupted(_) => "CORRUPTED",
            VaultError::Encryption(_) => "ENCRYPTION_ERROR",
            VaultError::Io(_) => "IO_ERROR",
            VaultError::Serialization(_) => "SERIALIZATION_ERROR",
            VaultError::Salt(_) => "SALT_ERROR",
            VaultError::KeyDerivation(_) => "KEY_DERIVATION_ERROR",
            VaultError::AlreadyExists => "ALREADY_EXISTS",
        };

        state.serialize_field("code", code)?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_error_to_decent_paste_error() {
        let vault_err = VaultError::InvalidPin;
        let app_err: crate::error::DecentPasteError = vault_err.into();
        assert!(matches!(
            app_err,
            crate::error::DecentPasteError::InvalidPin
        ));
    }

    #[test]
    fn test_vault_error_serialization() {
        let err = VaultError::InvalidPin;
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("INVALID_PIN"));
        assert!(json.contains("Invalid PIN"));
    }
}
