//! VaultManager - Core vault lifecycle management using IOTA Stronghold.
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
use tauri_plugin_stronghold::stronghold::Stronghold;
use tracing::{debug, error, info, warn};

use crate::clipboard::ClipboardEntry;
use crate::error::{DecentPasteError, Result};
use crate::storage::{get_data_dir, DeviceIdentity, PairedPeer};
use crate::vault::salt::{delete_salt, get_or_create_salt};

/// Argon2id parameters for key derivation.
/// These are chosen to balance security and usability:
/// - Memory: 64 MB (provides strong resistance to GPU attacks)
/// - Time: 3 iterations (reasonable delay on modern hardware)
/// - Parallelism: 4 lanes (utilizes multi-core CPUs)
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MB in KiB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_OUTPUT_LEN: usize = 32; // 256-bit key for AES-256

/// Vault file name
const VAULT_FILE_NAME: &str = "vault.hold";

/// Client name within the Stronghold vault
const VAULT_CLIENT_NAME: &str = "decentpaste";

/// Store keys for different data types
const STORE_KEY_CLIPBOARD_HISTORY: &[u8] = b"clipboard_history";
const STORE_KEY_PAIRED_PEERS: &[u8] = b"paired_peers";
const STORE_KEY_DEVICE_IDENTITY: &[u8] = b"device_identity";
const STORE_KEY_LIBP2P_KEYPAIR: &[u8] = b"libp2p_keypair";

/// VaultManager handles the lifecycle of the encrypted vault.
///
/// The vault uses IOTA Stronghold for secure storage, with the encryption
/// key derived from the user's PIN via Argon2id. This ensures:
/// - The PIN itself is never stored
/// - Each installation has a unique salt
/// - Strong resistance to brute-force attacks
pub struct VaultManager {
    /// The Stronghold instance (only present when vault is open)
    stronghold: Option<Stronghold>,
}

impl VaultManager {
    /// Create a new VaultManager instance.
    pub fn new() -> Self {
        Self { stronghold: None }
    }

    /// Get the path to the vault file.
    pub fn get_vault_path() -> Result<PathBuf> {
        let data_dir = get_data_dir()?;
        Ok(data_dir.join(VAULT_FILE_NAME))
    }

    /// Check if a vault file exists.
    ///
    /// Returns `true` if the vault has been set up previously.
    /// This is a fast, non-blocking check that doesn't require unlocking.
    pub fn exists() -> Result<bool> {
        let vault_path = Self::get_vault_path()?;
        Ok(vault_path.exists())
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
    /// A 32-byte key suitable for AES-256-GCM encryption.
    pub fn derive_key(pin: &str, salt: &[u8; 16]) -> Result<Vec<u8>> {
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
        let mut key = vec![0u8; ARGON2_OUTPUT_LEN];
        argon2
            .hash_password_into(pin.as_bytes(), salt, &mut key)
            .map_err(|e| DecentPasteError::Encryption(format!("Key derivation failed: {}", e)))?;

        debug!("Derived {}-byte key from PIN", key.len());
        Ok(key)
    }

    /// Create a new vault with the given PIN.
    ///
    /// This sets up a fresh Stronghold vault encrypted with a key derived
    /// from the PIN. Should only be called when no vault exists.
    ///
    /// # Arguments
    /// * `pin` - The user's chosen PIN (4-8 digits)
    ///
    /// # Errors
    /// Returns an error if a vault already exists or if creation fails.
    pub fn create(&mut self, pin: &str) -> Result<()> {
        let vault_path = Self::get_vault_path()?;

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

        // Initialize Stronghold with the derived key
        // Stronghold::new automatically creates a new vault if file doesn't exist
        let stronghold = Stronghold::new(&vault_path, key)
            .map_err(|e| DecentPasteError::Storage(format!("Failed to create vault: {}", e)))?;

        // Create the client within the vault for storing data
        stronghold
            .write_client(VAULT_CLIENT_NAME)
            .map_err(|e| DecentPasteError::Storage(format!("Failed to create vault client: {}", e)))?;

        // Save the vault to disk
        stronghold
            .save()
            .map_err(|e| DecentPasteError::Storage(format!("Failed to save vault: {}", e)))?;

        self.stronghold = Some(stronghold);

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
        let vault_path = Self::get_vault_path()?;

        if !vault_path.exists() {
            return Err(DecentPasteError::Storage("Vault does not exist".into()));
        }

        info!("Opening vault at {:?}", vault_path);

        // Get the salt (must exist if vault exists)
        let salt = get_or_create_salt()?;

        // Derive the key from PIN
        let key = Self::derive_key(pin, &salt)?;

        // Try to load the vault with the derived key
        // Stronghold::new will attempt to load the existing snapshot
        let stronghold = Stronghold::new(&vault_path, key).map_err(|e| {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("decrypt")
                || error_msg.contains("invalid")
                || error_msg.contains("authentication")
                || error_msg.contains("mac")
            {
                warn!("Invalid PIN attempt");
                DecentPasteError::InvalidPin
            } else {
                DecentPasteError::Storage(format!("Failed to open vault: {}", e))
            }
        })?;

        // Verify we can load the client (additional validation that vault opened correctly)
        stronghold.load_client(VAULT_CLIENT_NAME).map_err(|e| {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("decrypt") || error_msg.contains("not found") {
                // Client not found could mean corrupted vault or wrong key
                warn!("Could not load vault client - may be wrong PIN or corrupted");
                DecentPasteError::InvalidPin
            } else {
                DecentPasteError::Storage(format!("Failed to load vault client: {}", e))
            }
        })?;

        self.stronghold = Some(stronghold);

        info!("Vault opened successfully");
        Ok(())
    }

    /// Destroy the vault and all associated data.
    ///
    /// This is a destructive operation that:
    /// 1. Closes the vault if open
    /// 2. Deletes the vault file (vault.hold)
    /// 3. Deletes the salt file (salt.bin)
    ///
    /// After calling this, the app will need to go through onboarding again.
    ///
    /// # Warning
    /// All encrypted data will be permanently lost!
    pub fn destroy(&mut self) -> Result<()> {
        info!("Destroying vault - all data will be lost!");

        // Clear the stronghold reference first
        self.stronghold = None;

        // Delete vault file
        let vault_path = Self::get_vault_path()?;
        if vault_path.exists() {
            std::fs::remove_file(&vault_path)?;
            info!("Deleted vault file: {:?}", vault_path);
        }

        // Delete salt file
        delete_salt()?;
        info!("Deleted salt file");

        info!("Vault destroyed successfully");
        Ok(())
    }

    /// Check if the vault is currently open (unlocked).
    pub fn is_open(&self) -> bool {
        self.stronghold.is_some()
    }

    /// Get a reference to the Stronghold instance.
    ///
    /// Returns `None` if the vault is not open.
    pub fn stronghold(&self) -> Option<&Stronghold> {
        self.stronghold.as_ref()
    }

    /// Get a mutable reference to the Stronghold instance.
    ///
    /// Returns `None` if the vault is not open.
    pub fn stronghold_mut(&mut self) -> Option<&mut Stronghold> {
        self.stronghold.as_mut()
    }

    // =========================================================================
    // Data Operations - Clipboard History
    // =========================================================================

    /// Get clipboard history from the vault.
    ///
    /// Returns an empty vector if no history is stored or vault is not open.
    pub fn get_clipboard_history(&self) -> Result<Vec<ClipboardEntry>> {
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let store = stronghold.store();
        match store.get(STORE_KEY_CLIPBOARD_HISTORY) {
            Ok(Some(data)) => {
                let history: Vec<ClipboardEntry> = serde_json::from_slice(&data)?;
                debug!("Loaded {} clipboard entries from vault", history.len());
                Ok(history)
            }
            Ok(None) => {
                debug!("No clipboard history in vault");
                Ok(Vec::new())
            }
            Err(e) => {
                error!("Failed to get clipboard history: {}", e);
                Err(DecentPasteError::Storage(format!(
                    "Failed to get clipboard history: {}",
                    e
                )))
            }
        }
    }

    /// Set clipboard history in the vault.
    ///
    /// This overwrites any existing history. Call `flush()` to persist.
    pub fn set_clipboard_history(&self, history: &[ClipboardEntry]) -> Result<()> {
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let data = serde_json::to_vec(history)?;
        let store = stronghold.store();
        store
            .insert(STORE_KEY_CLIPBOARD_HISTORY.to_vec(), data, None)
            .map_err(|e| {
                DecentPasteError::Storage(format!("Failed to set clipboard history: {}", e))
            })?;

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
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let store = stronghold.store();
        match store.get(STORE_KEY_PAIRED_PEERS) {
            Ok(Some(data)) => {
                let peers: Vec<PairedPeer> = serde_json::from_slice(&data)?;
                debug!("Loaded {} paired peers from vault", peers.len());
                Ok(peers)
            }
            Ok(None) => {
                debug!("No paired peers in vault");
                Ok(Vec::new())
            }
            Err(e) => {
                error!("Failed to get paired peers: {}", e);
                Err(DecentPasteError::Storage(format!(
                    "Failed to get paired peers: {}",
                    e
                )))
            }
        }
    }

    /// Set paired peers in the vault.
    ///
    /// This overwrites any existing peers. Call `flush()` to persist.
    pub fn set_paired_peers(&self, peers: &[PairedPeer]) -> Result<()> {
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let data = serde_json::to_vec(peers)?;
        let store = stronghold.store();
        store
            .insert(STORE_KEY_PAIRED_PEERS.to_vec(), data, None)
            .map_err(|e| {
                DecentPasteError::Storage(format!("Failed to set paired peers: {}", e))
            })?;

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
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let store = stronghold.store();
        match store.get(STORE_KEY_DEVICE_IDENTITY) {
            Ok(Some(data)) => {
                let identity: DeviceIdentity = serde_json::from_slice(&data)?;
                debug!("Loaded device identity from vault: {}", identity.device_id);
                Ok(Some(identity))
            }
            Ok(None) => {
                debug!("No device identity in vault");
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get device identity: {}", e);
                Err(DecentPasteError::Storage(format!(
                    "Failed to get device identity: {}",
                    e
                )))
            }
        }
    }

    /// Set device identity in the vault.
    ///
    /// Call `flush()` to persist.
    pub fn set_device_identity(&self, identity: &DeviceIdentity) -> Result<()> {
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let data = serde_json::to_vec(identity)?;
        let store = stronghold.store();
        store
            .insert(STORE_KEY_DEVICE_IDENTITY.to_vec(), data, None)
            .map_err(|e| {
                DecentPasteError::Storage(format!("Failed to set device identity: {}", e))
            })?;

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
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let store = stronghold.store();
        match store.get(STORE_KEY_LIBP2P_KEYPAIR) {
            Ok(Some(data)) => {
                let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&data)
                    .map_err(|e| {
                        DecentPasteError::Storage(format!(
                            "Failed to decode libp2p keypair: {}",
                            e
                        ))
                    })?;
                debug!("Loaded libp2p keypair from vault");
                Ok(Some(keypair))
            }
            Ok(None) => {
                debug!("No libp2p keypair in vault");
                Ok(None)
            }
            Err(e) => {
                error!("Failed to get libp2p keypair: {}", e);
                Err(DecentPasteError::Storage(format!(
                    "Failed to get libp2p keypair: {}",
                    e
                )))
            }
        }
    }

    /// Set libp2p keypair in the vault.
    ///
    /// The keypair is stored in protobuf encoding. Call `flush()` to persist.
    pub fn set_libp2p_keypair(&self, keypair: &libp2p::identity::Keypair) -> Result<()> {
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        let data = keypair.to_protobuf_encoding().map_err(|e| {
            DecentPasteError::Storage(format!("Failed to encode libp2p keypair: {}", e))
        })?;

        let store = stronghold.store();
        store
            .insert(STORE_KEY_LIBP2P_KEYPAIR.to_vec(), data, None)
            .map_err(|e| {
                DecentPasteError::Storage(format!("Failed to set libp2p keypair: {}", e))
            })?;

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
        let stronghold = self.stronghold.as_ref().ok_or_else(|| {
            DecentPasteError::Storage("Vault is not open".into())
        })?;

        stronghold.save().map_err(|e| {
            error!("Failed to flush vault: {}", e);
            DecentPasteError::Storage(format!("Failed to flush vault: {}", e))
        })?;

        debug!("Vault flushed to disk");
        Ok(())
    }

    /// Lock the vault by flushing and clearing the Stronghold reference.
    ///
    /// This saves all data and clears the decryption key from memory.
    /// The vault file remains on disk but requires the PIN to open again.
    pub fn lock(&mut self) -> Result<()> {
        if let Some(ref stronghold) = self.stronghold {
            info!("Locking vault");

            // Flush before locking to ensure all data is saved
            if let Err(e) = stronghold.save() {
                warn!("Failed to save vault before locking: {}", e);
                // Continue with lock even if save fails
            }
        }

        self.stronghold = None;
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

        assert_eq!(key1, key2, "Same PIN and salt should produce same key");
        assert_eq!(key1.len(), ARGON2_OUTPUT_LEN, "Key should be 32 bytes");
    }

    #[test]
    fn test_derive_key_different_pins() {
        let salt = [1u8; 16];
        let key1 = VaultManager::derive_key("1234", &salt).unwrap();
        let key2 = VaultManager::derive_key("5678", &salt).unwrap();

        assert_ne!(key1, key2, "Different PINs should produce different keys");
    }

    #[test]
    fn test_derive_key_different_salts() {
        let salt1 = [1u8; 16];
        let salt2 = [2u8; 16];
        let key1 = VaultManager::derive_key("1234", &salt1).unwrap();
        let key2 = VaultManager::derive_key("1234", &salt2).unwrap();

        assert_ne!(key1, key2, "Different salts should produce different keys");
    }
}
