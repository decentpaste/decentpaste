use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

use chrono::{Duration, Utc};
use tokio::sync::{broadcast, mpsc, Notify, RwLock};
use tracing::{debug, warn};

use crate::clipboard::ClipboardEntry;
use crate::error::Result;
use crate::network::protocol::ClipboardMessage;
use crate::network::{DiscoveredPeer, NetworkCommand, NetworkStatus, PairingCodeRegistry};
use crate::security::PairingSession;
use crate::storage::{AppSettings, DeviceIdentity, PairedPeer};
use crate::vault::{VaultManager, VaultStatus};

/// Maximum number of messages to buffer per peer.
/// Set to 1 for "latest only" behavior - sync only delivers the most recent message.
pub const SYNC_MAX_BUFFER_SIZE: usize = 1;

/// Time-to-live for buffered messages in seconds.
/// Messages older than this are filtered out during sync.
/// 5 minutes is sufficient for typical offline durations (app restart, mobile background).
pub const SYNC_TTL_SECONDS: i64 = 60 * 5;

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

    // =========================================================================
    // Connection Management State
    // =========================================================================
    /// Guard against concurrent reconnection attempts.
    /// Only one ensure_connected() operation runs at a time.
    pub reconnect_in_progress: AtomicBool,

    /// Count of pending dial attempts during reconnection.
    /// Decremented as connections succeed or fail.
    pub pending_dials: AtomicUsize,

    /// Notified when all pending dials complete.
    /// Used by ensure_connected() to await completion.
    pub dials_complete_notify: Arc<Notify>,

    /// Per-recipient buffering: we store messages WE sent that THEY missed.
    /// Key = peer_id of the recipient (who missed the message)
    /// Value = messages we sent that they should receive on reconnection
    ///
    /// ALWAYS buffer for all paired peers, regardless of online status.
    /// This handles the race condition where a peer goes offline mid-transmission.
    /// Message buffers for offline peers.
    /// Maps peer_id -> buffered messages (messages WE sent that THEY missed).
    pub message_buffers: Arc<RwLock<HashMap<String, Vec<ClipboardMessage>>>>,

    // =========================================================================
    // Internet Pairing State
    // =========================================================================
    /// Registry of active internet pairing codes we've generated.
    /// Used to track codes we've shared and validate incoming connections.
    pub pairing_code_registry: Arc<RwLock<PairingCodeRegistry>>,

    /// Broadcast channel for peer connection events.
    /// Used by connect_with_pairing_code() to wait for a specific peer to connect
    /// instead of using a fixed sleep. The NetworkManager publishes to this when
    /// PeerConnected events fire.
    pub peer_connected_tx: broadcast::Sender<String>,

    /// Broadcast channel for relay connection events.
    /// Used by connect_with_pairing_code() to wait for relay connection before
    /// attempting to dial the peer. Payload is the relay peer ID.
    pub relay_connected_tx: broadcast::Sender<String>,
}

impl AppState {
    pub fn new() -> Self {
        // Create broadcast channels with capacity for 16 pending events.
        // This is enough for concurrent internet pairing attempts while avoiding
        // unbounded memory growth.
        let (peer_connected_tx, _) = broadcast::channel(16);
        let (relay_connected_tx, _) = broadcast::channel(16);

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

            // Connection management
            reconnect_in_progress: AtomicBool::new(false),
            pending_dials: AtomicUsize::new(0),
            dials_complete_notify: Arc::new(Notify::new()),

            // Sync message buffers (per-recipient)
            message_buffers: Arc::new(RwLock::new(HashMap::new())),

            // Internet pairing
            pairing_code_registry: Arc::new(RwLock::new(PairingCodeRegistry::new())),
            peer_connected_tx,
            relay_connected_tx,
        }
    }

    pub async fn add_clipboard_entry(&self, entry: ClipboardEntry) {
        let modified = {
            let mut history = self.clipboard_history.write().await;

            // Check for existing entry with the same content hash (deduplication)
            if let Some(idx) = history
                .iter()
                .position(|e| e.content_hash == entry.content_hash)
            {
                // Remove existing entry - it will be reinserted at the correct position
                history.remove(idx);
                debug!(
                    "Updated existing clipboard entry (will reinsert): {}",
                    &entry.content_hash[..8]
                );
            }

            // Insert at the correct chronological position by timestamp.
            // History is sorted newest-first, so find the first entry older than this one.
            // This is important for sync: synced messages may have older timestamps
            // and should appear in the correct position in history.
            let insert_pos = history
                .iter()
                .position(|e| e.timestamp < entry.timestamp)
                .unwrap_or(history.len());

            history.insert(insert_pos, entry);

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

    /// Store a clipboard message in buffer for a specific peer.
    /// ALWAYS buffers, regardless of peer's online status (handles race conditions).
    /// Buffer is per-recipient: messages WE sent that THEY missed.
    pub async fn store_buffered_message(&self, peer_id: &str, message: ClipboardMessage) {
        let mut buffers = self.message_buffers.write().await;
        let buffer = buffers.entry(peer_id.to_string()).or_default();
        buffer.push(message);

        // Truncate to max size (keep the latest messages only)
        if buffer.len() > SYNC_MAX_BUFFER_SIZE {
            buffer.drain(0..buffer.len() - SYNC_MAX_BUFFER_SIZE);
        }

        debug!(
            "Buffered message for peer {} (buffer size: {})",
            peer_id,
            buffer.len()
        );
    }

    /// Get buffered messages for a specific peer (read-only, does NOT remove).
    /// Filters out expired messages (older than SYNC_TTL_SECONDS).
    /// Used when building HashListResponse for sync.
    pub async fn get_buffer_for_peer(&self, peer_id: &str) -> Vec<ClipboardMessage> {
        let buffers = self.message_buffers.read().await;
        let now = Utc::now();
        let ttl = Duration::seconds(SYNC_TTL_SECONDS);

        buffers
            .get(peer_id)
            .map(|msgs| {
                msgs.iter()
                    .filter(|msg| now.signed_duration_since(msg.timestamp) < ttl)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find a message by content_hash in a SPECIFIC peer's buffer.
    /// Used when peer requests specific content via ContentRequest.
    /// Only searches the requesting peer's buffer to ensure correct encryption.
    pub async fn find_message_for_peer_by_hash(
        &self,
        peer_id: &str,
        hash: &str,
    ) -> Option<ClipboardMessage> {
        let buffers = self.message_buffers.read().await;
        buffers
            .get(peer_id)
            .and_then(|buffer| buffer.iter().find(|msg| msg.content_hash == hash).cloned())
    }

    /// Remove a specific message from a specific peer's buffer by content_hash.
    /// Called after peer successfully receives content via ContentResponse.
    pub async fn remove_buffered_message_for_peer(&self, peer_id: &str, hash: &str) {
        let mut buffers = self.message_buffers.write().await;
        if let Some(buffer) = buffers.get_mut(peer_id) {
            let before_len = buffer.len();
            buffer.retain(|msg| msg.content_hash != hash);
            if buffer.len() < before_len {
                debug!(
                    "Removed buffered message {} for peer {} (was delivered)",
                    &hash[..8.min(hash.len())],
                    peer_id
                );
            }
        }
    }

    /// Flush paired peers to vault immediately.
    ///
    /// This should be called after any mutation to paired_peers:
    /// - After pairing completes
    /// - After unpairing
    /// - After peer name updates
    pub async fn flush_paired_peers(&self) -> Result<()> {
        let mut vault_manager = self.vault_manager.write().await;
        if let Some(ref mut manager) = *vault_manager {
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

        let mut vault_manager = self.vault_manager.write().await;
        if let Some(ref mut manager) = *vault_manager {
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
        let mut vault_manager = self.vault_manager.write().await;
        if let Some(ref mut manager) = *vault_manager {
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
        let mut vault_manager = self.vault_manager.write().await;
        if let Some(ref mut manager) = *vault_manager {
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
