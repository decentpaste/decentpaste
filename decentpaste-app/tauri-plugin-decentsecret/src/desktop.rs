//! Desktop implementation using OS keyring.
//!
//! This module provides secure secret storage using platform-native keyrings:
//! - **macOS**: Keychain Access
//! - **Windows**: Credential Manager
//! - **Linux**: Secret Service API (GNOME Keyring, KWallet)

use keyring::Entry;
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use tracing::{debug, error, info, warn};

use crate::error::Error;
use crate::models::*;

/// Service name used for keyring entries.
const SERVICE_NAME: &str = "com.decentpaste.vault";

/// Account name (username) for the keyring entry.
const ACCOUNT_NAME: &str = "vault-key";

/// Initialize the desktop plugin.
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Decentsecret<R>> {
    Ok(Decentsecret(app.clone()))
}

/// Access to the decentsecret APIs for desktop platforms.
pub struct Decentsecret<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Decentsecret<R> {
    /// Check what secure storage capabilities are available.
    ///
    /// On desktop, we try to access the keyring to see if it's available.
    pub fn check_availability(&self) -> crate::Result<SecretStorageStatus> {
        debug!(
            "Checking keyring availability for service: {}",
            SERVICE_NAME
        );

        let entry = match Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
            Ok(entry) => entry,
            Err(e) => {
                warn!("Keyring not available: {}", e);
                return Ok(SecretStorageStatus::unavailable(format!(
                    "OS keyring not available: {}",
                    e
                )));
            }
        };
        let method = Self::get_platform_method();
        match entry.get_password() {
            Ok(_) => {
                debug!("Keyring available (entry exists), method: {:?}", method);
                Ok(SecretStorageStatus::available(method))
            }
            Err(keyring::Error::NoEntry) => {
                debug!("Keyring available (no entry yet), method: {:?}", method);
                Ok(SecretStorageStatus::available(method))
            }
            Err(e) => {
                warn!("Keyring not accessible: {:?}", e);
                Ok(SecretStorageStatus::unavailable(format!(
                    "OS keyring not accessible: {}",
                    e
                )))
            }
        }
    }

    /// Store a secret in the OS keyring.
    ///
    /// The secret is stored as base64-encoded bytes to handle binary data safely.
    pub fn store_secret(&self, secret: Vec<u8>) -> crate::Result<()> {
        info!(
            "Attempting to store {} byte secret in keyring (service: {}, account: {})",
            secret.len(),
            SERVICE_NAME,
            ACCOUNT_NAME
        );

        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| {
            error!("Failed to create keyring entry: {}", e);
            Self::map_keyring_error(e)
        })?;

        // Encode as base64 for safe storage (keyring APIs expect strings)
        let encoded = base64_encode(&secret);
        debug!("Encoded secret length: {} chars", encoded.len());

        match entry.set_password(&encoded) {
            Ok(()) => {
                info!("set_password() returned Ok");
            }
            Err(e) => {
                error!("Failed to store secret in keyring: {:?}", e);
                return Err(Self::map_keyring_error(e));
            }
        }

        // Verify the secret was actually stored by creating a NEW Entry and reading back
        // This ensures we're not just reading a cached value from the original Entry
        let verify_entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| {
            error!("Failed to create verification entry: {}", e);
            Self::map_keyring_error(e)
        })?;

        match verify_entry.get_password() {
            Ok(readback) => {
                if readback == encoded {
                    info!("Secret verified with new Entry - successfully stored in OS keyring");
                    Ok(())
                } else {
                    error!("Secret verification failed - stored data doesn't match!");
                    Err(Error::Internal(
                        "Keyring verification failed: data mismatch".into(),
                    ))
                }
            }
            Err(e) => {
                error!(
                    "Secret verification failed - cannot read back with new Entry: {:?}",
                    e
                );
                Err(Error::Internal(format!(
                    "Keyring verification failed: set_password() succeeded but get_password() on new Entry failed: {:?}",
                    e
                )))
            }
        }
    }

    /// Retrieve the secret from the OS keyring.
    pub fn retrieve_secret(&self) -> crate::Result<Vec<u8>> {
        debug!(
            "Attempting to retrieve secret from keyring (service: {}, account: {})",
            SERVICE_NAME, ACCOUNT_NAME
        );

        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| {
            error!("Failed to create keyring entry for retrieval: {}", e);
            Self::map_keyring_error(e)
        })?;

        let encoded = match entry.get_password() {
            Ok(password) => {
                debug!("Retrieved encoded secret, length: {} chars", password.len());
                password
            }
            Err(e) => {
                error!("Failed to retrieve secret from keyring: {:?}", e);
                return Err(Self::map_keyring_error(e));
            }
        };

        let secret = base64_decode(&encoded).map_err(|e| {
            error!("Failed to decode secret from base64: {}", e);
            Error::Internal(format!("Failed to decode secret: {}", e))
        })?;

        info!(
            "Secret successfully retrieved from OS keyring ({} bytes)",
            secret.len()
        );
        Ok(secret)
    }

    /// Delete the secret from the OS keyring.
    pub fn delete_secret(&self) -> crate::Result<()> {
        debug!(
            "Attempting to delete secret from keyring (service: {}, account: {})",
            SERVICE_NAME, ACCOUNT_NAME
        );

        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| {
            error!("Failed to create keyring entry for deletion: {}", e);
            Self::map_keyring_error(e)
        })?;

        // delete_credential returns an error if the entry doesn't exist,
        // but we want delete to be idempotent
        match entry.delete_credential() {
            Ok(()) => {
                info!("Secret deleted from OS keyring");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                debug!("No secret to delete (already gone)");
                Ok(())
            }
            Err(e) => {
                error!("Failed to delete secret from keyring: {:?}", e);
                Err(Self::map_keyring_error(e))
            }
        }
    }

    /// Get the appropriate storage method for the current platform.
    fn get_platform_method() -> SecretStorageMethod {
        #[cfg(target_os = "macos")]
        {
            SecretStorageMethod::MacOSKeychain
        }
        #[cfg(target_os = "windows")]
        {
            SecretStorageMethod::WindowsCredentialManager
        }
        #[cfg(target_os = "linux")]
        {
            SecretStorageMethod::LinuxSecretService
        }
        // Fallback for other platforms (shouldn't happen on desktop)
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            SecretStorageMethod::LinuxSecretService
        }
    }

    /// Map keyring errors to our error type.
    fn map_keyring_error(err: keyring::Error) -> Error {
        match err {
            keyring::Error::NoEntry => Error::SecretNotFound,
            keyring::Error::Ambiguous(_) => {
                Error::Internal("Multiple keyring entries found".into())
            }
            keyring::Error::NoStorageAccess(e) => {
                Error::NotAvailable(format!("Keyring access denied: {:?}", e))
            }
            keyring::Error::PlatformFailure(e) => {
                let msg = format!("{:?}", e);
                if msg.contains("Dbus") || msg.contains("dbus") || msg.contains("D-Bus") {
                    Error::NotAvailable(format!(
                        "System keyring not available (D-Bus error): {}",
                        msg
                    ))
                } else {
                    Error::Internal(format!("Keyring error: {:?}", e))
                }
            }
            keyring::Error::BadEncoding(e) => {
                Error::Internal(format!("Keyring encoding error: {:?}", e))
            }
            _ => Error::Internal(format!("Keyring error: {}", err)),
        }
    }
}

/// Base64 encode bytes to string.
fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Base64 decode string to bytes.
fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| e.to_string())
}
