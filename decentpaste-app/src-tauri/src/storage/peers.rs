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
    /// Stored in encrypted vault.
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
    /// Last known network addresses for this peer (from mDNS discovery).
    /// Used as fallback when mDNS hasn't rediscovered the peer yet.
    /// Stored as strings (Multiaddr format) for serialization compatibility.
    #[serde(default)]
    pub last_known_addresses: Vec<String>,

    // Internet connectivity fields
    /// Relay addresses through which this peer can be reached.
    /// Used when direct connection fails (peer behind NAT).
    /// Format: Full relay circuit address (e.g., "/ip4/.../p2p/RELAY/p2p-circuit/p2p/PEER")
    #[serde(default)]
    pub relay_addresses: Vec<String>,
    /// Whether this peer was originally paired via internet (vs local mDNS).
    /// Used to determine reconnection strategy.
    #[serde(default)]
    pub paired_via_internet: bool,
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
