//! Tauri plugin for secure secret storage across all platforms.
//!
//! This plugin provides a unified API for storing secrets using:
//! - **Android**: AndroidKeyStore with BiometricPrompt (TEE/StrongBox)
//! - **iOS**: Keychain with Secure Enclave
//! - **macOS**: Keychain Access
//! - **Windows**: Credential Manager
//! - **Linux**: Secret Service API (GNOME Keyring, KWallet)

use tauri::{
    plugin::{self, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(desktop)]
use desktop::Decentsecret;
#[cfg(mobile)]
use mobile::Decentsecret;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the decentsecret APIs.
pub trait DecentsecretExt<R: Runtime> {
    fn decentsecret(&self) -> &Decentsecret<R>;
}

impl<R: Runtime, T: Manager<R>> DecentsecretExt<R> for T {
    fn decentsecret(&self) -> &Decentsecret<R> {
        self.state::<Decentsecret<R>>().inner()
    }
}

/// Default desktop keyring service name.
///
/// Desktop keyrings (macOS Keychain, Windows Credential Manager, Linux Secret
/// Service) are shared across the whole user account, so this value is used to
/// namespace DecentPaste's entries. Embedding apps can override it via
/// [`Builder::service_name`] to avoid colliding with DecentPaste's entry.
///
/// This is a desktop-only concern: on mobile the OS already scopes secret
/// storage to the app (per-UID AndroidKeyStore/SharedPreferences, per-app iOS
/// Keychain), so no service name is needed there.
const DEFAULT_SERVICE_NAME: &str = "com.decentpaste.vault";

/// Builder for the decentsecret plugin.
///
/// Use this when the default desktop keyring service name
/// (`com.decentpaste.vault`) is not appropriate — for example when another
/// application embeds this plugin and must not collide with DecentPaste's entry
/// in the user-global desktop keyring:
///
/// ```ignore
/// tauri::Builder::default()
///     .plugin(
///         tauri_plugin_decentsecret::Builder::new()
///             .service_name("com.example.myapp.vault")
///             .build(),
///     )
/// ```
///
/// The service name only affects desktop platforms; on mobile the OS scopes
/// secret storage to the app, so the value is ignored there.
pub struct Builder {
    service_name: String,
}

impl Builder {
    /// Create a new builder using the default service name (`com.decentpaste.vault`).
    pub fn new() -> Self {
        Self {
            service_name: DEFAULT_SERVICE_NAME.to_string(),
        }
    }

    /// Override the desktop keyring service name.
    ///
    /// Ignored on mobile, where the OS scopes secret storage to the app.
    pub fn service_name(mut self, service_name: impl Into<String>) -> Self {
        self.service_name = service_name.into();
        self
    }

    /// Build the plugin.
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        let service_name = self.service_name;
        // The service name configures the desktop keyring only; on mobile the OS
        // scopes secret storage to the app, so the value is unused there.
        #[cfg(mobile)]
        let _ = service_name;

        plugin::Builder::new("decentsecret")
            .invoke_handler(tauri::generate_handler![
                commands::check_availability,
                commands::store_secret,
                commands::retrieve_secret,
                commands::delete_secret,
            ])
            .setup(move |app, api| {
                #[cfg(mobile)]
                let decentsecret = mobile::init(app, api)?;
                #[cfg(desktop)]
                let decentsecret = desktop::init(app, api, service_name)?;
                app.manage(decentsecret);
                Ok(())
            })
            .build()
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// Initializes the plugin with the default desktop keyring service name.
///
/// Add this to your Tauri app builder:
/// ```ignore
/// tauri::Builder::default()
///     .plugin(tauri_plugin_decentsecret::init())
/// ```
///
/// To customize the desktop keyring service name, use [`Builder`] instead. The
/// service name only applies on desktop; mobile secret stores are scoped to the
/// app by the OS.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
