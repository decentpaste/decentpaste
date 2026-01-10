//! VaultManager - Core vault lifecycle management using AES-256-GCM encryption.
//!
//! This module provides the VaultManager struct that handles:
//! - Vault existence checking
//! - Vault creation with PIN-derived encryption keys
//! - Vault opening (unlocking) with PIN verification
//! - Vault destruction for factory reset
//! - Encrypted storage for clipboard history, paired peers, device identity, and keypairs
//!
//! The encryption key is derived from the user's PIN using Argon2id with
//! installation-specific salt, providing strong protection against brute-force attacks.

use std::path::PathBuf;

use argon2::{Algorithm, Argon2, Params, Version};
use tracing::{debug, info, warn};

use crate::clipboard::ClipboardEntry;
use crate::error::{DecentPasteError, Result};
use crate::storage::{DeviceIdentity, PairedPeer};
use crate::vault::salt::{delete_salt, get_or_create_salt};
use crate::vault::storage::{
    delete_vault, get_vault_path, read_vault, vault_exists, write_vault, VaultData, VaultKey,
};

/// Argon2id parameters for key derivation.
/// These are chosen to balance security and usability:
/// - Memory: 64 MB (provides strong resistance to GPU attacks)
/// - Time: 3 iterations (reasonable delay on modern hardware)
/// - Parallelism: 4 lanes (utilizes multi-core CPUs)
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MB in KiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 32; // 256-bit key for AES-256

/// VaultManager handles the lifecycle of the encrypted vault.
///
/// The vault uses AES-256-GCM for secure storage, with the encryption
/// key derived from the user's PIN via Argon2id. This ensures:
/// - The PIN itself is never stored
/// - Each installation has a unique salt
/// - Strong resistance to brute-force attacks
pub struct VaultManager {
    /// The derived encryption key (only present when vault is open)
    key: Option<VaultKey>,
    /// In-memory vault data (loaded when vault is opened)
    data: VaultData,
}

impl VaultManager {
    /// Create a new VaultManager instance.
    pub fn new() -> Self {
        Self {
            key: None,
            data: VaultData::default(),
        }
    }

    /// Get the path to the vault file.
    pub fn get_vault_path() -> Result<PathBuf> {
        get_vault_path()
    }

    /// Check if a vault file exists.
    ///
    /// Returns `true` if the vault has been set up previously.
    /// This is a fast, non-blocking check that doesn't require unlocking.
    pub fn exists() -> Result<bool> {
        vault_exists()
    }

    /// Derive an encryption key from the PIN using Argon2id.
    ///
    /// This is the core security function that transforms the user's PIN
    /// into a strong encryption key. The derivation is intentionally slow
    /// to resist brute-force attacks.
    ///
    /// # Arguments
    /// * `pin` - The user's PIN (4-8 digits)
    /// * `salt` - Installation-specific 16-byte salt
    ///
    /// # Returns
    /// A VaultKey suitable for AES-256-GCM encryption.
    pub fn derive_key(pin: &str, salt: &[u8; 16]) -> Result<VaultKey> {
        // Configure Argon2id with our security parameters
        let params = Params::new(
            ARGON2_MEMORY_COST,
            ARGON2_TIME_COST,
            ARGON2_PARALLELISM,
            Some(ARGON2_OUTPUT_LEN),
        )
        .map_err(|e| DecentPasteError::Encryption(format!("Invalid Argon2 params: {}", e)))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        // Derive the key
        let mut key_bytes = [0u8; ARGON2_OUTPUT_LEN];
        argon2
            .hash_password_into(pin.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| DecentPasteError::Encryption(format!("Key derivation failed: {}", e)))?;

        debug!("Derived {}-byte key from PIN", key_bytes.len());
        Ok(VaultKey::from_slice(&key_bytes))
    }

    /// Create a new vault with the given PIN.
    ///
    /// This sets up a fresh encrypted vault with a key derived
    /// from the PIN. Should only be called when no vault exists.
    ///
    /// # Arguments
    /// * `pin` - The user's chosen PIN (4-8 digits)
    ///
    /// # Errors
    /// Returns an error if a vault already exists or if creation fails.
    pub fn create(&mut self, pin: &str) -> Result<()> {
        let vault_path = get_vault_path()?;

        if vault_path.exists() {
            return Err(DecentPasteError::Storage(
                "Vault already exists. Use destroy() first to reset.".into(),
            ));
        }

        info!("Creating new vault at {:?}", vault_path);

        // Get or create installation-specific salt
        let salt = get_or_create_salt()?;

        // Derive encryption key from PIN
        let key = Self::derive_key(pin, &salt)?;

        // Initialize empty vault data
        let data = VaultData::default();

        // Write encrypted vault to disk
        write_vault(&data, &key)?;

        self.key = Some(key);
        self.data = data;

        info!("Vault created successfully");
        Ok(())
    }

    /// Open an existing vault with the given PIN.
    ///
    /// Attempts to decrypt the vault using the provided PIN. If the PIN
    /// is incorrect, the decryption will fail.
    ///
    /// # Arguments
    /// * `pin` - The user's PIN
    ///
    /// # Errors
    /// Returns `InvalidPin` if the PIN is incorrect, or other errors
    /// if the vault file is corrupted or inaccessible.
    pub fn open(&mut self, pin: &str) -> Result<()> {
        let vault_path = get_vault_path()?;

        if !vault_path.exists() {
            return Err(DecentPasteError::Storage("Vault does not exist".into()));
        }

        info!("Opening vault at {:?}", vault_path);

        // Get the salt (must exist if vault exists)
        let salt = get_or_create_salt()?;

        // Derive the key from PIN
        let key = Self::derive_key(pin, &salt)?;

        // Try to decrypt the vault
        let data = read_vault(&key)?;

        self.key = Some(key);
        self.data = data;

        info!("Vault opened successfully");
        Ok(())
    }

    /// Destroy the vault and all associated data.
    ///
    /// This is a destructive operation that:
    /// 1. Closes the vault if open
    /// 2. Deletes the vault file (vault.enc)
    /// 3. Deletes the salt file (salt.bin)
    ///
    /// After calling this, the app will need to go through onboarding again.
    ///
    /// # Warning
    /// All encrypted data will be permanently lost!
    pub fn destroy(&mut self) -> Result<()> {
        info!("Destroying vault - all data will be lost!");

        // Clear the key and data from memory
        self.key = None;
        self.data = VaultData::default();

        // Delete vault file
        delete_vault()?;
        info!("Deleted vault file");

        // Delete salt file
        delete_salt()?;
        info!("Deleted salt file");

        info!("Vault destroyed successfully");
        Ok(())
    }

    /// Check if the vault is currently open (unlocked).
    pub fn is_open(&self) -> bool {
        self.key.is_some()
    }

    // =========================================================================
    // Data Operations - Clipboard History
    // =========================================================================

    /// Get clipboard history from the vault.
    ///
    /// Returns an empty vector if no history is stored or vault is not open.
    pub fn get_clipboard_history(&self) -> Result<Vec<ClipboardEntry>> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        Ok(self.data.clipboard_history.clone())
    }

    /// Set clipboard history in the vault.
    ///
    /// This updates the in-memory data. Call `flush()` to persist.
    pub fn set_clipboard_history(&mut self, history: &[ClipboardEntry]) -> Result<()> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        self.data.clipboard_history = history.to_vec();
        debug!("Stored {} clipboard entries in vault", history.len());
        Ok(())
    }

    // =========================================================================
    // Data Operations - Paired Peers
    // =========================================================================

    /// Get paired peers from the vault.
    ///
    /// Returns an empty vector if no peers are stored or vault is not open.
    pub fn get_paired_peers(&self) -> Result<Vec<PairedPeer>> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        Ok(self.data.paired_peers.clone())
    }

    /// Set paired peers in the vault.
    ///
    /// This updates the in-memory data. Call `flush()` to persist.
    pub fn set_paired_peers(&mut self, peers: &[PairedPeer]) -> Result<()> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        self.data.paired_peers = peers.to_vec();
        debug!("Stored {} paired peers in vault", peers.len());
        Ok(())
    }

    // =========================================================================
    // Data Operations - Device Identity
    // =========================================================================

    /// Get device identity from the vault.
    ///
    /// Returns `None` if no identity is stored or vault is not open.
    pub fn get_device_identity(&self) -> Result<Option<DeviceIdentity>> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        Ok(self.data.device_identity.clone())
    }

    /// Set device identity in the vault.
    ///
    /// Call `flush()` to persist.
    pub fn set_device_identity(&mut self, identity: &DeviceIdentity) -> Result<()> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }
        self.data.device_identity = Some(identity.clone());
        debug!("Stored device identity in vault: {}", identity.device_id);
        Ok(())
    }

    // =========================================================================
    // Data Operations - libp2p Keypair
    // =========================================================================

    /// Get libp2p keypair from the vault.
    ///
    /// Returns `None` if no keypair is stored or vault is not open.
    /// The keypair is stored in protobuf encoding.
    pub fn get_libp2p_keypair(&self) -> Result<Option<libp2p::identity::Keypair>> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }

        match &self.data.libp2p_keypair {
            Some(data) => {
                let keypair =
                    libp2p::identity::Keypair::from_protobuf_encoding(data).map_err(|e| {
                        DecentPasteError::Storage(format!("Failed to decode libp2p keypair: {}", e))
                    })?;
                debug!("Loaded libp2p keypair from vault");
                Ok(Some(keypair))
            }
            None => {
                debug!("No libp2p keypair in vault");
                Ok(None)
            }
        }
    }

    /// Set libp2p keypair in the vault.
    ///
    /// The keypair is stored in protobuf encoding. Call `flush()` to persist.
    pub fn set_libp2p_keypair(&mut self, keypair: &libp2p::identity::Keypair) -> Result<()> {
        if !self.is_open() {
            return Err(DecentPasteError::Storage("Vault is not open".into()));
        }

        let data = keypair.to_protobuf_encoding().map_err(|e| {
            DecentPasteError::Storage(format!("Failed to encode libp2p keypair: {}", e))
        })?;
        self.data.libp2p_keypair = Some(data);
        debug!("Stored libp2p keypair in vault");
        Ok(())
    }

    // =========================================================================
    // Persistence Operations
    // =========================================================================

    /// Flush all in-memory data to the encrypted vault file.
    ///
    /// This saves the current state of the vault to disk. Should be called:
    /// - Before locking the vault
    /// - When the app goes to background (mobile)
    /// - Periodically to prevent data loss
    /// - Before app exit
    pub fn flush(&self) -> Result<()> {
        let key = self
            .key
            .as_ref()
            .ok_or_else(|| DecentPasteError::Storage("Vault is not open".into()))?;

        write_vault(&self.data, key)?;
        debug!("Vault flushed to disk");
        Ok(())
    }

    /// Lock the vault by flushing and clearing the key from memory.
    ///
    /// This saves all data and clears the decryption key from memory.
    /// The vault file remains on disk but requires the PIN to open again.
    pub fn lock(&mut self) -> Result<()> {
        if self.is_open() {
            info!("Locking vault");

            // Flush before locking to ensure all data is saved
            if let Err(e) = self.flush() {
                warn!("Failed to save vault before locking: {}", e);
                // Continue with lock even if save fails
            }
        }

        // Clear key (VaultKey implements ZeroizeOnDrop, so memory is securely erased)
        self.key = None;
        // Keep data in memory but it can't be persisted without key
        Ok(())
    }
}

impl Default for VaultManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let salt = [1u8; 16];
        let key1 = VaultManager::derive_key("1234", &salt).unwrap();
        let key2 = VaultManager::derive_key("1234", &salt).unwrap();

        assert_eq!(
            key1.as_bytes(),
            key2.as_bytes(),
            "Same PIN and salt should produce same key"
        );
        assert_eq!(
            key1.as_bytes().len(),
            ARGON2_OUTPUT_LEN,
            "Key should be 32 bytes"
        );
    }

    #[test]
    fn test_derive_key_different_pins() {
        let salt = [1u8; 16];
        let key1 = VaultManager::derive_key("1234", &salt).unwrap();
        let key2 = VaultManager::derive_key("5678", &salt).unwrap();

        assert_ne!(
            key1.as_bytes(),
            key2.as_bytes(),
            "Different PINs should produce different keys"
        );
    }

    #[test]
    fn test_derive_key_different_salts() {
        let salt1 = [1u8; 16];
        let salt2 = [2u8; 16];
        let key1 = VaultManager::derive_key("1234", &salt1).unwrap();
        let key2 = VaultManager::derive_key("1234", &salt2).unwrap();

        assert_ne!(
            key1.as_bytes(),
            key2.as_bytes(),
            "Different salts should produce different keys"
        );
    }
}
