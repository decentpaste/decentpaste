//! Auth method persistence for vault configuration.
//!
//! Stores the authentication method (SecureStorage or Pin) in a JSON file
//! so the app knows which unlock path to use before decrypting the vault.
//! This file is not sensitive - it only indicates which auth method was chosen.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::auth::AuthMethod;
use crate::error::{DecentPasteError, Result};
use crate::storage::get_data_dir;

/// File name for auth method configuration.
const AUTH_METHOD_FILE: &str = "auth-method.json";

/// Configuration structure stored in the JSON file.
#[derive(Debug, Serialize, Deserialize)]
struct AuthMethodConfig {
    method: AuthMethod,
}

/// Get the path to the auth method config file.
fn get_auth_method_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join(AUTH_METHOD_FILE))
}

/// Load the stored auth method.
///
/// Returns `None` if the config file doesn't exist (no vault set up yet).
///
/// # Errors
/// Returns an error if the file exists but cannot be read or parsed.
pub fn load_auth_method() -> Result<Option<AuthMethod>> {
    let path = get_auth_method_path()?;

    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path).map_err(|e| {
        DecentPasteError::Storage(format!("Failed to read auth method file: {}", e))
    })?;

    let config: AuthMethodConfig = serde_json::from_str(&content).map_err(|e| {
        DecentPasteError::Storage(format!("Failed to parse auth method file: {}", e))
    })?;

    Ok(Some(config.method))
}

/// Save the auth method to disk.
///
/// Creates or overwrites the config file with the given auth method.
///
/// # Errors
/// Returns an error if the file cannot be written.
pub fn save_auth_method(method: AuthMethod) -> Result<()> {
    let path = get_auth_method_path()?;
    let config = AuthMethodConfig { method };

    let content = serde_json::to_string_pretty(&config).map_err(|e| {
        DecentPasteError::Storage(format!("Failed to serialize auth method: {}", e))
    })?;

    std::fs::write(&path, content).map_err(|e| {
        DecentPasteError::Storage(format!("Failed to write auth method file: {}", e))
    })?;

    Ok(())
}

/// Delete the auth method config file.
///
/// Called during vault reset to ensure clean state.
/// Returns `Ok(())` if the file doesn't exist (idempotent).
///
/// # Errors
/// Returns an error if the file exists but cannot be deleted.
pub fn delete_auth_method() -> Result<()> {
    let path = get_auth_method_path()?;

    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| {
            DecentPasteError::Storage(format!("Failed to delete auth method file: {}", e))
        })?;
    }

    Ok(())
}
