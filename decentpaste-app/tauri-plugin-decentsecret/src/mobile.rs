//! Mobile implementation bridging to native Android/iOS code.
//!
//! This module provides the Rust bridge that calls into:
//! - **Android**: Kotlin plugin using AndroidKeyStore + BiometricPrompt
//! - **iOS**: Swift plugin using Secure Enclave + LocalAuthentication

use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::error::Error;
use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_decentsecret);

/// Initialize the mobile plugin by registering native code.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Decentsecret<R>> {
    #[cfg(target_os = "android")]
    let handle =
        api.register_android_plugin("com.decentpaste.plugins.decentsecret", "DecentsecretPlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_decentsecret)?;
    Ok(Decentsecret(handle))
}

/// Access to the decentsecret APIs for mobile platforms.
pub struct Decentsecret<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Decentsecret<R> {
    /// Check what secure storage capabilities are available.
    ///
    /// Calls native code to check biometric hardware availability.
    pub fn check_availability(&self) -> crate::Result<SecretStorageStatus> {
        self.0
            .run_mobile_plugin("checkAvailability", ())
            .map_err(|e| self.map_plugin_error(e))
    }

    /// Store a secret using biometric-protected hardware storage.
    ///
    /// - **Android**: Shows BiometricPrompt, encrypts with TEE key
    /// - **iOS**: Stores in Keychain with Secure Enclave protection
    pub fn store_secret(&self, secret: Vec<u8>) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("storeSecret", StoreSecretRequest { secret })
            .map_err(|e| self.map_plugin_error(e))
    }

    /// Retrieve the secret from biometric-protected storage.
    ///
    /// - **Android**: Shows BiometricPrompt, decrypts with TEE key
    /// - **iOS**: Shows Face ID/Touch ID, retrieves from Secure Enclave
    pub fn retrieve_secret(&self) -> crate::Result<Vec<u8>> {
        let response: RetrieveSecretResponse = self
            .0
            .run_mobile_plugin("retrieveSecret", ())
            .map_err(|e| self.map_plugin_error(e))?;
        Ok(response.secret)
    }

    /// Delete the secret from biometric-protected storage.
    pub fn delete_secret(&self) -> crate::Result<()> {
        self.0
            .run_mobile_plugin("deleteSecret", ())
            .map_err(|e| self.map_plugin_error(e))
    }

    /// Map native plugin errors to our error type.
    ///
    /// Native code returns structured errors that we parse here.
    fn map_plugin_error(&self, err: tauri::plugin::mobile::PluginInvokeError) -> Error {
        let msg = err.to_string();

        // Parse error codes from native plugins
        if msg.contains("NOT_AVAILABLE") {
            Error::NotAvailable(msg)
        } else if msg.contains("AUTH_FAILED") {
            Error::AuthenticationFailed(msg)
        } else if msg.contains("BIOMETRIC_CHANGED") {
            Error::BiometricEnrollmentChanged
        } else if msg.contains("NO_BIOMETRICS") {
            Error::NoBiometricsEnrolled
        } else if msg.contains("NOT_FOUND") {
            Error::SecretNotFound
        } else if msg.contains("ACCESS_DENIED") {
            Error::AccessDenied
        } else if msg.contains("USER_CANCELLED") {
            Error::UserCancelled
        } else {
            Error::PluginInvoke(msg)
        }
    }
}
