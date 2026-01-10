//! Desktop implementation using OS keyring.
//!
//! This module provides secure secret storage using platform-native keyrings:
//! - **macOS**: Keychain Access
//! - **Windows**: Credential Manager
//! - **Linux**: Secret Service API (GNOME Keyring, KWallet)

use keyring::Entry;
use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

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
        // Try to create a keyring entry to check if the service is available
        match Entry::new(SERVICE_NAME, ACCOUNT_NAME) {
            Ok(_) => {
                let method = Self::get_platform_method();
                log::debug!("Keyring available, method: {:?}", method);
                Ok(SecretStorageStatus::available(method))
            }
            Err(e) => {
                log::warn!("Keyring not available: {}", e);
                Ok(SecretStorageStatus::unavailable(format!(
                    "OS keyring not available: {}",
                    e
                )))
            }
        }
    }

    /// Store a secret in the OS keyring.
    ///
    /// The secret is stored as base64-encoded bytes to handle binary data safely.
    pub fn store_secret(&self, secret: Vec<u8>) -> crate::Result<()> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| Error::Internal(format!("Failed to create keyring entry: {}", e)))?;

        // Encode as base64 for safe storage (keyring APIs expect strings)
        let encoded = base64_encode(&secret);

        entry
            .set_password(&encoded)
            .map_err(|e| Self::map_keyring_error(e))?;

        log::info!("Secret stored in OS keyring");
        Ok(())
    }

    /// Retrieve the secret from the OS keyring.
    pub fn retrieve_secret(&self) -> crate::Result<Vec<u8>> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| Error::Internal(format!("Failed to create keyring entry: {}", e)))?;

        let encoded = entry
            .get_password()
            .map_err(|e| Self::map_keyring_error(e))?;

        let secret = base64_decode(&encoded)
            .map_err(|e| Error::Internal(format!("Failed to decode secret: {}", e)))?;

        log::debug!("Secret retrieved from OS keyring");
        Ok(secret)
    }

    /// Delete the secret from the OS keyring.
    pub fn delete_secret(&self) -> crate::Result<()> {
        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| Error::Internal(format!("Failed to create keyring entry: {}", e)))?;

        // delete_credential returns an error if the entry doesn't exist,
        // but we want delete to be idempotent
        match entry.delete_credential() {
            Ok(()) => {
                log::info!("Secret deleted from OS keyring");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                log::debug!("No secret to delete (already gone)");
                Ok(())
            }
            Err(e) => Err(Self::map_keyring_error(e)),
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
            keyring::Error::PlatformFailure(e) => Error::Internal(format!("Keyring error: {:?}", e)),
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
