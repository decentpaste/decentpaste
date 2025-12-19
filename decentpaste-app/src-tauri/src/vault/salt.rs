//! Salt manager for Argon2 key derivation.
//!
//! Each installation generates a unique 16-byte salt that is used
//! with Argon2id to derive the vault encryption key from the user's PIN.
//! The salt is stored in `salt.bin` and persists across app restarts.

use rand::rngs::OsRng;
use rand::RngCore;
use std::path::PathBuf;

use crate::error::{DecentPasteError, Result};
use crate::storage::get_data_dir;

/// Salt size in bytes (128 bits)
const SALT_SIZE: usize = 16;

/// Get the path to the salt file.
fn get_salt_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("salt.bin"))
}

/// Get the existing salt or create a new one.
///
/// The salt is a cryptographically random 16-byte value used as input
/// to Argon2id along with the user's PIN to derive the vault key.
/// Using a unique salt per installation ensures that identical PINs
/// produce different keys on different devices.
///
/// # Returns
/// A 16-byte salt array.
///
/// # Errors
/// Returns an error if the data directory is not initialized or if
/// file operations fail.
pub fn get_or_create_salt() -> Result<[u8; SALT_SIZE]> {
    let path = get_salt_path()?;

    if path.exists() {
        // Load existing salt
        let bytes = std::fs::read(&path)?;
        if bytes.len() != SALT_SIZE {
            return Err(DecentPasteError::Storage(format!(
                "Invalid salt file size: expected {} bytes, got {}",
                SALT_SIZE,
                bytes.len()
            )));
        }

        let mut salt = [0u8; SALT_SIZE];
        salt.copy_from_slice(&bytes);
        return Ok(salt);
    }

    // Generate new cryptographically secure salt
    let mut salt = [0u8; SALT_SIZE];
    OsRng.fill_bytes(&mut salt);

    // Save to disk
    std::fs::write(&path, &salt)?;

    // Set restrictive permissions on Unix (salt is sensitive)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(salt)
}

/// Delete the salt file.
///
/// This is called during vault reset to ensure a fresh salt is generated
/// when the user sets up a new vault. Without deleting the salt, the old
/// vault could theoretically be recovered if the PIN was known.
///
/// # Errors
/// Returns an error if the file exists but cannot be deleted.
/// Returns Ok(()) if the file doesn't exist.
pub fn delete_salt() -> Result<()> {
    let path = get_salt_path()?;

    if path.exists() {
        std::fs::remove_file(&path)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_salt_size() {
        assert_eq!(SALT_SIZE, 16);
    }
}
