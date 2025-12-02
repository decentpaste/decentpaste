use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{DecentPasteError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
    pub public_key: Vec<u8>,
    #[serde(skip_serializing, skip_deserializing)]
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

pub fn get_data_dir() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "decentpaste", "app")
        .ok_or_else(|| DecentPasteError::Storage("Could not determine data directory".into()))?;

    let data_dir = proj_dirs.data_dir();
    std::fs::create_dir_all(data_dir)?;
    Ok(data_dir.to_path_buf())
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
