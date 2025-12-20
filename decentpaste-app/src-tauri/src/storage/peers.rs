use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;
use tauri::{AppHandle, Manager};

use crate::error::{DecentPasteError, Result};

/// Static storage for the data directory path, initialized once from Tauri
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    pub public_key: Vec<u8>,
    /// X25519 private key for ECDH key derivation during pairing.
    /// Now stored in encrypted vault (previously skipped for plaintext storage).
    pub private_key: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedPeer {
    pub peer_id: String,
    pub device_name: String,
    pub shared_secret: Vec<u8>,
    pub paired_at: DateTime<Utc>,
    pub last_seen: Option<DateTime<Utc>>,
}

/// Initialize the data directory using Tauri's path resolver.
/// Must be called once at app startup before any storage operations.
pub fn init_data_dir(app: &AppHandle) -> Result<()> {
    let data_dir = app.path().app_data_dir().map_err(|e| {
        DecentPasteError::Storage(format!("Could not determine data directory: {}", e))
    })?;

    std::fs::create_dir_all(&data_dir)?;

    DATA_DIR
        .set(data_dir)
        .map_err(|_| DecentPasteError::Storage("Data directory already initialized".into()))?;

    Ok(())
}

pub fn get_data_dir() -> Result<PathBuf> {
    DATA_DIR.get().cloned().ok_or_else(|| {
        DecentPasteError::Storage(
            "Data directory not initialized. Call init_data_dir first.".into(),
        )
    })
}

fn get_peers_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("peers.json"))
}

fn get_identity_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("identity.json"))
}

fn get_private_key_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("private_key.bin"))
}

fn get_libp2p_keypair_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("libp2p_keypair.bin"))
}

/// Load or create the libp2p keypair for consistent PeerId across restarts
pub fn get_or_create_libp2p_keypair() -> Result<libp2p::identity::Keypair> {
    let path = get_libp2p_keypair_path()?;

    if path.exists() {
        // Load existing keypair using protobuf encoding
        let bytes = std::fs::read(&path)?;
        let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&bytes).map_err(|e| {
            DecentPasteError::Storage(format!("Failed to load libp2p keypair: {}", e))
        })?;
        return Ok(keypair);
    }

    // Generate new keypair
    let keypair = libp2p::identity::Keypair::generate_ed25519();

    // Save to disk using protobuf encoding
    let bytes = keypair.to_protobuf_encoding().map_err(|e| {
        DecentPasteError::Storage(format!("Failed to encode libp2p keypair: {}", e))
    })?;
    std::fs::write(&path, &bytes)?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(keypair)
}

pub fn load_paired_peers() -> Result<Vec<PairedPeer>> {
    let path = get_peers_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&path)?;
    let peers: Vec<PairedPeer> = serde_json::from_str(&content)?;
    Ok(peers)
}

pub fn save_paired_peers(peers: &[PairedPeer]) -> Result<()> {
    let path = get_peers_path()?;
    let content = serde_json::to_string_pretty(peers)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn load_device_identity() -> Result<Option<DeviceIdentity>> {
    let identity_path = get_identity_path()?;
    let private_key_path = get_private_key_path()?;

    if !identity_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&identity_path)?;
    let mut identity: DeviceIdentity = serde_json::from_str(&content)?;

    // Load private key separately
    if private_key_path.exists() {
        identity.private_key = Some(std::fs::read(&private_key_path)?);
    }

    Ok(Some(identity))
}

pub fn save_device_identity(identity: &DeviceIdentity) -> Result<()> {
    let identity_path = get_identity_path()?;
    let private_key_path = get_private_key_path()?;

    // Save identity (without private key)
    let content = serde_json::to_string_pretty(identity)?;
    std::fs::write(&identity_path, content)?;

    // Save private key separately with restricted permissions
    if let Some(ref private_key) = identity.private_key {
        std::fs::write(&private_key_path, private_key)?;

        // Set restrictive permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&private_key_path)?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&private_key_path, perms)?;
        }
    }

    Ok(())
}
