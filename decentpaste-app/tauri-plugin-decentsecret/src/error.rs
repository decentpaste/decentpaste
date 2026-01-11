//! Error types for the decentsecret plugin.

use serde::{Deserialize, Serialize};

/// Result type alias for plugin operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during secret storage operations.
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "type", content = "message")]
pub enum Error {
    /// Secure storage is not available on this platform/device.
    #[error("Secure storage not available: {0}")]
    NotAvailable(String),

    /// User failed to authenticate (wrong biometric, cancelled, etc.)
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Biometric enrollment changed since the secret was stored.
    /// On mobile, this invalidates the key - vault must be reset.
    #[error("Biometric enrollment changed - stored secrets are now inaccessible")]
    BiometricEnrollmentChanged,

    /// No biometrics enrolled on the device.
    #[error("No biometrics enrolled on this device")]
    NoBiometricsEnrolled,

    /// No secret is stored (nothing to retrieve/delete).
    #[error("No secret found in secure storage")]
    SecretNotFound,

    /// Access to secure storage was denied by the OS.
    #[error("Access denied to secure storage")]
    AccessDenied,

    /// User cancelled the biometric prompt.
    #[error("User cancelled authentication")]
    UserCancelled,

    /// I/O error during keyring operations.
    #[error("I/O error: {0}")]
    Io(String),

    /// Platform-specific internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Mobile plugin invocation error.
    #[cfg(mobile)]
    #[error("Plugin invoke error: {0}")]
    PluginInvoke(String),
}

// Implement conversion from mobile plugin errors
#[cfg(mobile)]
impl From<tauri::plugin::mobile::PluginInvokeError> for Error {
    fn from(err: tauri::plugin::mobile::PluginInvokeError) -> Self {
        Error::PluginInvoke(err.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string())
    }
}
