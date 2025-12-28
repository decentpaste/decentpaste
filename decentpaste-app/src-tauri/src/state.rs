use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, warn};

use crate::clipboard::ClipboardEntry;
use crate::error::Result;
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
    /// Clipboard content received while app was in background (mobile only)
    /// This is processed when app resumes to foreground
    pub pending_clipboard: Arc<RwLock<Option<PendingClipboard>>>,
    /// Whether the app is currently in foreground (tracked for mobile)
    pub is_foreground: Arc<RwLock<bool>>,
    /// Peers confirmed ready to receive broadcast messages.
    /// This is protocol-agnostic - the network layer determines what "ready" means.
    /// Currently: gossipsub topic subscription. Future: could be any protocol.
    pub ready_peers: Arc<RwLock<HashSet<String>>>,
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
            pending_clipboard: Arc::new(RwLock::new(None)),
            is_foreground: Arc::new(RwLock::new(true)), // Assume foreground at start
            ready_peers: Arc::new(RwLock::new(HashSet::new())), // No peers ready initially
            vault_status: Arc::new(RwLock::new(VaultStatus::NotSetup)), // Vault starts as not setup
            vault_manager: Arc::new(RwLock::new(None)), // No vault manager until unlocked
        }
    }

    pub async fn add_clipboard_entry(&self, entry: ClipboardEntry) {
        let modified = {
            let mut history = self.clipboard_history.write().await;

            // Check for existing entry with same content hash
            if let Some(idx) = history
                .iter()
                .position(|e| e.content_hash == entry.content_hash)
            {
                // Update existing entry with new metadata and move to front
                // This keeps history clean while allowing re-sharing same content
                history.remove(idx);
                debug!(
                    "Updated existing clipboard entry (moved to front): {}",
                    &entry.content_hash[..8]
                );
            }

            // Add to front (either new entry or updated existing)
            history.insert(0, entry);

            // Trim to max size from settings
            let max_size = self.settings.read().await.clipboard_history_limit;
            history.truncate(max_size);
            true
        };

        // Flush-on-write: persist clipboard history immediately
        if modified {
            if let Err(e) = self.flush_clipboard_history().await {
                warn!("Failed to flush clipboard history: {}", e);
            }
        }
    }

    pub async fn is_peer_paired(&self, peer_id: &str) -> bool {
        let peers = self.paired_peers.read().await;
        peers.iter().any(|p| p.peer_id == peer_id)
    }

    // =========================================================================
    // Vault Flush Helpers - Flush-on-Write Pattern
    // =========================================================================
    //
    // These methods implement the flush-on-write pattern for data persistence.
    // Each method updates the vault and immediately flushes to disk, ensuring
    // data is never lost even on unexpected termination (crashes, force quit,
    // macOS Cmd+Q, etc.).

    /// Flush paired peers to vault immediately.
    ///
    /// This should be called after any mutation to paired_peers:
    /// - After pairing completes
    /// - After unpairing
    /// - After peer name updates
    pub async fn flush_paired_peers(&self) -> Result<()> {
        let vault_manager = self.vault_manager.read().await;
        if let Some(ref manager) = *vault_manager {
            let peers = self.paired_peers.read().await;
            manager.set_paired_peers(&peers)?;
            manager.flush()?;
            debug!("Flushed {} paired peers to vault", peers.len());
            Ok(())
        } else {
            warn!("Cannot flush paired peers: vault not open");
            Ok(()) // Don't error - vault might not be open yet
        }
    }

    /// Flush clipboard history to vault immediately.
    ///
    /// This should be called after any mutation to clipboard_history:
    /// - After adding a new entry
    /// - After clearing history
    ///
    /// Note: Only flushes if `keep_history` setting is enabled.
    pub async fn flush_clipboard_history(&self) -> Result<()> {
        let keep_history = self.settings.read().await.keep_history;
        if !keep_history {
            return Ok(()); // History persistence disabled
        }

        let vault_manager = self.vault_manager.read().await;
        if let Some(ref manager) = *vault_manager {
            let history = self.clipboard_history.read().await;
            manager.set_clipboard_history(&history)?;
            manager.flush()?;
            debug!("Flushed {} clipboard entries to vault", history.len());
            Ok(())
        } else {
            warn!("Cannot flush clipboard history: vault not open");
            Ok(())
        }
    }

    /// Flush device identity to vault immediately.
    ///
    /// This should be called after device identity changes:
    /// - After device name update in settings
    pub async fn flush_device_identity(&self) -> Result<()> {
        let vault_manager = self.vault_manager.read().await;
        if let Some(ref manager) = *vault_manager {
            let identity = self.device_identity.read().await;
            if let Some(ref id) = *identity {
                manager.set_device_identity(id)?;
                manager.flush()?;
                debug!("Flushed device identity to vault: {}", id.device_id);
            }
            Ok(())
        } else {
            warn!("Cannot flush device identity: vault not open");
            Ok(())
        }
    }

    /// Flush all vault data immediately.
    ///
    /// This is a convenience method that flushes all data types.
    /// Used primarily as a safety net in lifecycle handlers.
    pub async fn flush_all_to_vault(&self) -> Result<()> {
        let vault_manager = self.vault_manager.read().await;
        if let Some(ref manager) = *vault_manager {
            let keep_history = self.settings.read().await.keep_history;

            // Flush clipboard history if enabled
            if keep_history {
                let history = self.clipboard_history.read().await;
                if let Err(e) = manager.set_clipboard_history(&history) {
                    warn!("Failed to set clipboard history in vault: {}", e);
                }
            }

            // Always flush paired peers
            let peers = self.paired_peers.read().await;
            if let Err(e) = manager.set_paired_peers(&peers) {
                warn!("Failed to set paired peers in vault: {}", e);
            }

            // Flush to disk
            manager.flush()?;
            debug!("Flushed all data to vault");
            Ok(())
        } else {
            warn!("Cannot flush all to vault: vault not open");
            Ok(())
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
