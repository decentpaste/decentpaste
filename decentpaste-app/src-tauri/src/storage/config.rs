use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::peers::get_data_dir;
use crate::error::Result;

/// Default relay servers for internet connectivity.
///
/// Format: Multiaddr with peer ID, e.g.:
/// - `/ip4/1.2.3.4/tcp/4001/p2p/12D3KooW...`
/// - `/dns4/relay.example.com/tcp/4001/p2p/12D3KooW...`
///
/// Note: Peer ID is required for relay connections.
///
/// For local testing, run the relay server and get its Peer ID from /info endpoint:
/// ```bash
/// cd decentpaste-relay && cargo run
/// curl http://localhost:8080/info  # Get the peer_id
/// ```
pub const DEFAULT_RELAY_SERVERS: &[&str] = &[
    // Public libp2p bootstrap nodes (support circuit relay v2)
    // These are run by Protocol Labs / IPFS Foundation
    // Note: May have rate limits - use dedicated relays for production
    // See: https://docs.ipfs.tech/concepts/public-utilities/
    // Updated 2026-01-27: Peer ID was stale, using correct one now
    "/ip4/xx.xx.xx.xx/tcp/4001/p2p/12D3KooWGPxpmwDLnJwLJAueeG5yDAJRcXZbHekCDd5rTLbv1DTs",
];

/// Application settings stored in settings.json.
///
/// Note: This struct uses `#[serde(default)]` for backward compatibility.
/// Old settings files missing new fields will use defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub device_name: String,
    pub auto_sync_enabled: bool,
    pub clipboard_history_limit: usize,
    /// Whether to persist clipboard history across app restarts.
    /// When true, history is saved to the encrypted vault.
    /// When false, history is cleared on exit.
    pub keep_history: bool,
    pub clipboard_poll_interval_ms: u64,
    /// Preferred authentication method for vault access.
    /// Currently only "pin" is supported. None means not yet configured (onboarding).
    pub auth_method: Option<String>,
    /// Whether to hide clipboard content in the UI (privacy mode).
    /// When true, content is masked with dots.
    pub hide_clipboard_content: bool,
    /// Auto-lock timeout in minutes. 0 means never auto-lock.
    pub auto_lock_minutes: u32,

    // Internet connectivity settings
    /// Whether internet sync is enabled (connect via relay servers)
    pub internet_sync_enabled: bool,
    /// Custom relay servers (in addition to or instead of defaults)
    /// Format: Multiaddr strings, e.g., "/dns4/relay.example.com/tcp/4001/p2p/12D3..."
    pub relay_servers: Vec<String>,
    /// Whether to use default relay servers (true) or only custom ones (false)
    pub use_default_relays: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            device_name: get_default_device_name(),
            auto_sync_enabled: true,
            clipboard_history_limit: 50,
            keep_history: true,
            clipboard_poll_interval_ms: 500,
            auth_method: None,
            hide_clipboard_content: false,
            auto_lock_minutes: 15,
            // Internet connectivity defaults
            internet_sync_enabled: false,
            relay_servers: DEFAULT_RELAY_SERVERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            use_default_relays: true,
        }
    }
}

fn get_default_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown Device".to_string())
}

fn get_settings_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("settings.json"))
}

pub fn load_settings() -> Result<AppSettings> {
    let path = get_settings_path()?;

    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = std::fs::read_to_string(&path)?;
    let settings: AppSettings = serde_json::from_str(&content)?;
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = get_settings_path()?;
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;
    Ok(())
}
