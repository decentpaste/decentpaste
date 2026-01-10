//! Encrypted vault file storage using AES-256-GCM.
//!
//! This module provides:
//! - `VaultKey`: A 256-bit key wrapper that zeroizes on drop
//! - `VaultData`: Serializable struct containing all vault contents
//! - Functions to read/write the encrypted `vault.enc` file

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::clipboard::ClipboardEntry;
use crate::error::{DecentPasteError, Result};
use crate::storage::{get_data_dir, DeviceIdentity, PairedPeer};

/// Nonce size for AES-GCM (96 bits = 12 bytes)
const NONCE_SIZE: usize = 12;

/// Vault file name
pub const VAULT_FILE_NAME: &str = "vault.enc";

/// A 256-bit encryption key with automatic zeroization on drop.
///
/// This wrapper ensures that the key material is securely erased from memory
/// when the vault is locked or the application exits.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct VaultKey {
    key: [u8; 32],
}

impl VaultKey {
    /// Create a new VaultKey from a 32-byte slice.
    ///
    /// # Panics
    /// Panics if the slice is not exactly 32 bytes.
    pub fn from_slice(slice: &[u8]) -> Self {
        let mut key = [0u8; 32];
        key.copy_from_slice(slice);
        Self { key }
    }

    /// Get the key as a byte slice for cryptographic operations.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

impl std::fmt::Debug for VaultKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never log the actual key material
        f.debug_struct("VaultKey")
            .field("key", &"[REDACTED]")
            .finish()
    }
}

/// All data stored in the encrypted vault.
///
/// This struct is serialized to JSON, then encrypted with AES-256-GCM.
/// The nonce is prepended to the ciphertext when written to disk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultData {
    /// Clipboard history entries
    #[serde(default)]
    pub clipboard_history: Vec<ClipboardEntry>,

    /// Paired peer information including shared secrets
    #[serde(default)]
    pub paired_peers: Vec<PairedPeer>,

    /// This device's identity and keypair
    #[serde(default)]
    pub device_identity: Option<DeviceIdentity>,

    /// libp2p Ed25519 keypair (protobuf-encoded)
    #[serde(default)]
    pub libp2p_keypair: Option<Vec<u8>>,
}

/// Get the path to the vault file.
pub fn get_vault_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join(VAULT_FILE_NAME))
}

/// Check if a vault file exists.
pub fn vault_exists() -> Result<bool> {
    let vault_path = get_vault_path()?;
    Ok(vault_path.exists())
}

/// Encrypt and write vault data to disk.
///
/// Format: `[12-byte nonce][ciphertext with 16-byte auth tag]`
pub fn write_vault(data: &VaultData, key: &VaultKey) -> Result<()> {
    let vault_path = get_vault_path()?;

    // Serialize to JSON
    let plaintext = serde_json::to_vec(data)?;

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| DecentPasteError::Encryption(format!("Invalid key: {}", e)))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| DecentPasteError::Encryption(format!("Encryption failed: {}", e)))?;

    // Prepend nonce to ciphertext
    let mut output = nonce_bytes.to_vec();
    output.extend(ciphertext);

    // Write atomically (write to temp file, then rename)
    let temp_path = vault_path.with_extension("enc.tmp");
    std::fs::write(&temp_path, &output)?;
    std::fs::rename(&temp_path, &vault_path)?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&vault_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&vault_path, perms)?;
    }

    Ok(())
}

/// Read and decrypt vault data from disk.
///
/// Returns an error if the file doesn't exist, is corrupted, or the key is wrong.
pub fn read_vault(key: &VaultKey) -> Result<VaultData> {
    let vault_path = get_vault_path()?;

    // Read file
    let encrypted = std::fs::read(&vault_path)?;

    if encrypted.len() < NONCE_SIZE {
        return Err(DecentPasteError::Storage("Vault file too short".into()));
    }

    // Extract nonce and ciphertext
    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Create cipher
    let cipher = Aes256Gcm::new_from_slice(key.as_bytes())
        .map_err(|e| DecentPasteError::Encryption(format!("Invalid key: {}", e)))?;

    // Decrypt
    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        // Decryption failure = wrong key (invalid PIN) or corrupted file
        DecentPasteError::InvalidPin
    })?;

    // Deserialize JSON
    let data: VaultData =
        serde_json::from_slice(&plaintext).map_err(|e| DecentPasteError::Storage(format!("Vault data corrupted: {}", e)))?;

    Ok(data)
}

/// Delete the vault file.
pub fn delete_vault() -> Result<()> {
    let vault_path = get_vault_path()?;
    if vault_path.exists() {
        std::fs::remove_file(&vault_path)?;
    }
    Ok(())
}
