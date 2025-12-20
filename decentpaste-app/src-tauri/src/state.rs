use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use crate::clipboard::ClipboardEntry;
use crate::network::{DiscoveredPeer, NetworkCommand, NetworkStatus};
use crate::security::PairingSession;
use crate::storage::{AppSettings, DeviceIdentity, PairedPeer};
use crate::vault::{VaultManager, VaultStatus};

/// Clipboard content received while app was in background (Android)
#[derive(Debug, Clone)]
pub struct PendingClipboard {
    pub content: String,
    pub from_device: String,
}

pub struct AppState {
    pub device_identity: Arc<RwLock<Option<DeviceIdentity>>>,
    pub settings: Arc<RwLock<AppSettings>>,
    pub paired_peers: Arc<RwLock<Vec<PairedPeer>>>,
    pub discovered_peers: Arc<RwLock<Vec<DiscoveredPeer>>>,
    pub clipboard_history: Arc<RwLock<Vec<ClipboardEntry>>>,
    pub network_status: Arc<RwLock<NetworkStatus>>,
    pub pairing_sessions: Arc<RwLock<Vec<PairingSession>>>,
    pub network_command_tx: Arc<RwLock<Option<mpsc::Sender<NetworkCommand>>>>,
    pub last_clipboard_hash: Arc<RwLock<Option<String>>>,
    /// Clipboard content received while app was in background (Android only)
    /// This is processed when app resumes to foreground
    pub pending_clipboard: Arc<RwLock<Option<PendingClipboard>>>,
    /// Whether the app is currently in foreground (tracked for mobile)
    pub is_foreground: Arc<RwLock<bool>>,
    /// Current vault authentication status
    pub vault_status: Arc<RwLock<VaultStatus>>,
    /// VaultManager instance for encrypted storage (only present when vault is open)
    pub vault_manager: Arc<RwLock<Option<VaultManager>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            device_identity: Arc::new(RwLock::new(None)),
            settings: Arc::new(RwLock::new(AppSettings::default())),
            paired_peers: Arc::new(RwLock::new(Vec::new())),
            discovered_peers: Arc::new(RwLock::new(Vec::new())),
            clipboard_history: Arc::new(RwLock::new(Vec::new())),
            network_status: Arc::new(RwLock::new(NetworkStatus::Disconnected)),
            pairing_sessions: Arc::new(RwLock::new(Vec::new())),
            network_command_tx: Arc::new(RwLock::new(None)),
            last_clipboard_hash: Arc::new(RwLock::new(None)),
            pending_clipboard: Arc::new(RwLock::new(None)),
            is_foreground: Arc::new(RwLock::new(true)), // Assume foreground at start
            vault_status: Arc::new(RwLock::new(VaultStatus::NotSetup)), // Vault starts as not setup
            vault_manager: Arc::new(RwLock::new(None)), // No vault manager until unlocked
        }
    }

    pub async fn add_clipboard_entry(&self, entry: ClipboardEntry) {
        let mut history = self.clipboard_history.write().await;

        // Check for duplicates by hash
        if history.iter().any(|e| e.content_hash == entry.content_hash) {
            return;
        }

        // Add to front
        history.insert(0, entry);

        // Trim to max size from settings
        let max_size = self.settings.read().await.clipboard_history_limit;
        history.truncate(max_size);
    }

    pub async fn get_paired_peer_by_id(&self, peer_id: &str) -> Option<PairedPeer> {
        let peers = self.paired_peers.read().await;
        peers.iter().find(|p| p.peer_id == peer_id).cloned()
    }

    pub async fn is_peer_paired(&self, peer_id: &str) -> bool {
        let peers = self.paired_peers.read().await;
        peers.iter().any(|p| p.peer_id == peer_id)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
