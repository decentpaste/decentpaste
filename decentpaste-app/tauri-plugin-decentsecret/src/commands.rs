//! Tauri command handlers for the decentsecret plugin.

use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::DecentsecretExt;
use crate::Result;

/// Check what secure storage capabilities are available on this platform.
///
/// Returns information about:
/// - Whether secure storage is available
/// - Which method will be used (biometric, keychain, etc.)
/// - Why it's unavailable (if applicable)
#[command]
pub(crate) async fn check_availability<R: Runtime>(app: AppHandle<R>) -> Result<SecretStorageStatus> {
    app.decentsecret().check_availability()
}

/// Store a secret in platform secure storage.
///
/// - **Android**: Wraps with biometric-protected key in AndroidKeyStore (TEE/StrongBox)
/// - **iOS**: Stores in Keychain with Secure Enclave protection
/// - **Desktop**: Stores in OS keyring (Keychain/Credential Manager/Secret Service)
#[command]
pub(crate) async fn store_secret<R: Runtime>(
    app: AppHandle<R>,
    request: StoreSecretRequest,
) -> Result<()> {
    app.decentsecret().store_secret(request.secret)
}

/// Retrieve the secret from platform secure storage.
///
/// - **Android**: Shows BiometricPrompt, unwraps with TEE
/// - **iOS**: Shows Face ID/Touch ID, retrieves from Secure Enclave
/// - **Desktop**: Retrieves from OS keyring (no prompt, session-based)
#[command]
pub(crate) async fn retrieve_secret<R: Runtime>(app: AppHandle<R>) -> Result<RetrieveSecretResponse> {
    let secret = app.decentsecret().retrieve_secret()?;
    Ok(RetrieveSecretResponse { secret })
}

/// Delete the secret from platform secure storage.
///
/// Used during vault reset or when the user wants to switch auth methods.
#[command]
pub(crate) async fn delete_secret<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.decentsecret().delete_secret()
}
