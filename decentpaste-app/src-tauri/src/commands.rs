use std::sync::atomic::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri::State;
use tracing::{debug, info, warn};

use crate::clipboard::ClipboardEntry;
use crate::error::{DecentPasteError, Result};
use crate::network::{DiscoveredPeer, NetworkCommand, NetworkStatus};
use crate::security::{generate_pin, PairingSession, PairingState};
use crate::state::{AppState, ConnectionStatus, PeerConnectionState};
use crate::storage::{save_settings, AppSettings, PairedPeer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub peer_id: Option<String>,
}

// Network commands
#[tauri::command]
pub async fn get_network_status(state: State<'_, AppState>) -> Result<NetworkStatus> {
    let status = state.network_status.read().await;
    Ok(status.clone())
}

#[tauri::command]
pub async fn start_network(state: State<'_, AppState>) -> Result<()> {
    let tx = state.network_command_tx.read().await;
    if let Some(tx) = tx.as_ref() {
        tx.send(NetworkCommand::StartListening)
            .await
            .map_err(|_| DecentPasteError::ChannelSend)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn stop_network(state: State<'_, AppState>) -> Result<()> {
    let tx = state.network_command_tx.read().await;
    if let Some(tx) = tx.as_ref() {
        tx.send(NetworkCommand::StopListening)
            .await
            .map_err(|_| DecentPasteError::ChannelSend)?;
    }
    Ok(())
}

/// Force reconnection to all discovered and paired peers.
/// Call this when the app resumes from background on mobile.
/// Uses paired peers' last-known addresses as fallback when mDNS hasn't rediscovered them.
#[tauri::command]
pub async fn reconnect_peers(state: State<'_, AppState>) -> Result<()> {
    // Get paired peers with their last-known addresses for reconnection fallback
    let paired_peer_addresses: Vec<(String, Vec<String>)> = {
        let peers = state.paired_peers.read().await;
        peers
            .iter()
            .filter(|p| !p.last_known_addresses.is_empty())
            .map(|p| (p.peer_id.clone(), p.last_known_addresses.clone()))
            .collect()
    };

    let tx = state.network_command_tx.read().await;
    if let Some(tx) = tx.as_ref() {
        tx.send(NetworkCommand::ReconnectPeers {
            paired_peer_addresses,
        })
        .await
        .map_err(|_| DecentPasteError::ChannelSend)?;
    }
    Ok(())
}

/// Update app visibility state (called from frontend on visibility change).
/// This ensures backend is the single source of truth for foreground state.
#[tauri::command]
pub async fn set_app_visibility(state: State<'_, AppState>, visible: bool) -> Result<()> {
    let mut fg = state.is_foreground.write().await;
    *fg = visible;
    debug!("App visibility set to: {}", visible);
    Ok(())
}

/// Response from process_pending_clipboard
#[derive(Debug, Clone, Serialize)]
pub struct PendingClipboardResponse {
    pub content: String,
    pub from_device: String,
}

/// Process any pending clipboard content that was received while app was in background.
/// Call this when the app becomes visible on mobile (from visibilitychange event).
/// Returns the pending clipboard content if any was waiting.
#[tauri::command]
pub async fn process_pending_clipboard(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<PendingClipboardResponse>> {
    use tracing::info;

    // Mark as foreground
    {
        let mut fg = state.is_foreground.write().await;
        *fg = true;
    }

    // Take pending clipboard if any
    let pending = {
        let mut p = state.pending_clipboard.write().await;
        p.take()
    };

    if let Some(pending) = pending {
        info!(
            "Processing pending clipboard from {} ({} chars)",
            pending.from_device,
            pending.content.len()
        );

        // Try to copy to clipboard
        if let Err(e) =
            crate::clipboard::monitor::set_clipboard_content(&app_handle, &pending.content)
        {
            tracing::error!("Failed to set pending clipboard: {}", e);
            return Err(DecentPasteError::Clipboard(e.to_string()));
        }

        info!("Pending clipboard copied successfully");
        Ok(Some(PendingClipboardResponse {
            content: pending.content,
            from_device: pending.from_device,
        }))
    } else {
        Ok(None)
    }
}

// Peer management
#[tauri::command]
pub async fn get_discovered_peers(state: State<'_, AppState>) -> Result<Vec<DiscoveredPeer>> {
    let discovered = state.discovered_peers.read().await;
    let paired = state.paired_peers.read().await;

    // Filter out peers that are already paired
    let filtered: Vec<DiscoveredPeer> = discovered
        .iter()
        .filter(|d| !paired.iter().any(|p| p.peer_id == d.peer_id))
        .cloned()
        .collect();

    Ok(filtered)
}

#[tauri::command]
pub async fn get_paired_peers(state: State<'_, AppState>) -> Result<Vec<PairedPeer>> {
    let peers = state.paired_peers.read().await;
    Ok(peers.clone())
}

#[tauri::command]
pub async fn remove_paired_peer(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<()> {
    use crate::network::DiscoveredPeer;
    use chrono::Utc;
    use tauri::Emitter;

    // Get the peer info before removing (we'll use it to re-emit as discovered)
    let peer_info = {
        let peers = state.paired_peers.read().await;
        peers
            .iter()
            .find(|p| p.peer_id == peer_id)
            .map(|p| (p.peer_id.clone(), p.device_name.clone()))
    };

    // Remove from paired list and flush to vault immediately
    {
        let mut peers = state.paired_peers.write().await;
        peers.retain(|p| p.peer_id != peer_id);
    }
    // Flush-on-write: persist immediately to prevent data loss
    state.flush_paired_peers().await?;

    // Emit directly using the info we have from the paired peer
    // This ensures the peer appears in discovered list with correct device name
    if let Some((pid, device_name)) = peer_info {
        let discovered = DiscoveredPeer {
            peer_id: pid.clone(),
            device_name: Some(device_name),
            addresses: vec![], // We don't have addresses, but that's okay for display
            discovered_at: Utc::now(),
            is_paired: false,
        };

        // Add to discovered peers state
        {
            let mut peers = state.discovered_peers.write().await;
            if !peers.iter().any(|p| p.peer_id == pid) {
                peers.push(discovered.clone());
            }
        }

        // Emit to frontend
        let _ = app_handle.emit("peer-discovered", discovered);
    }

    Ok(())
}

// Pairing flow
#[tauri::command]
pub async fn initiate_pairing(state: State<'_, AppState>, peer_id: String) -> Result<String> {
    // Check if already paired
    if state.is_peer_paired(&peer_id).await {
        return Err(DecentPasteError::AlreadyPaired(peer_id));
    }

    // Capture peer addresses NOW before mDNS can expire during pairing flow
    let peer_addresses = {
        let discovered = state.discovered_peers.read().await;
        discovered
            .iter()
            .find(|p| p.peer_id == peer_id)
            .map(|p| p.addresses.clone())
            .unwrap_or_default()
    };

    // Create pairing session with cached addresses
    let session_id = uuid::Uuid::new_v4().to_string();
    let session = PairingSession::new(session_id.clone(), peer_id.clone(), true)
        .with_peer_addresses(peer_addresses);

    let mut sessions = state.pairing_sessions.write().await;
    sessions.push(session);

    // Send pairing request through network
    let device_identity = state.device_identity.read().await;
    if let Some(ref identity) = *device_identity {
        let tx = state.network_command_tx.read().await;
        if let Some(tx) = tx.as_ref() {
            let request = crate::network::PairingRequest {
                session_id: session_id.clone(), // Include session_id so responder uses the same one
                device_name: identity.device_name.clone(),
                device_id: identity.device_id.clone(),
                public_key: identity.public_key.clone(),
            };

            let message = crate::network::ProtocolMessage::Pairing(
                crate::network::protocol::PairingMessage::Request(request),
            );

            tx.send(NetworkCommand::SendPairingRequest {
                peer_id,
                message: message.to_bytes().unwrap_or_default(),
            })
            .await
            .map_err(|_| DecentPasteError::ChannelSend)?;
        }
    }

    Ok(session_id)
}

#[tauri::command]
pub async fn respond_to_pairing(
    state: State<'_, AppState>,
    session_id: String,
    accept: bool,
) -> Result<Option<String>> {
    let peer_id: String;
    let pin_result: Option<String>;

    {
        let mut sessions = state.pairing_sessions.write().await;

        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            // Guard against duplicate calls - if already processed, return existing PIN
            if session.state == PairingState::AwaitingPinConfirmation {
                tracing::debug!("respond_to_pairing called again for already-accepted session, returning existing PIN");
                return Ok(session.pin.clone());
            }
            if matches!(
                session.state,
                PairingState::Failed(_)
                    | PairingState::Completed
                    | PairingState::AwaitingPeerConfirmation
            ) {
                tracing::debug!(
                    "respond_to_pairing called for session in terminal state: {:?}",
                    session.state
                );
                return Err(DecentPasteError::Pairing(
                    "Session already processed".into(),
                ));
            }

            peer_id = session.peer_id.clone();

            if accept {
                // Generate PIN
                let pin = generate_pin();
                session.pin = Some(pin.clone());
                session.state = PairingState::AwaitingPinConfirmation;
                pin_result = Some(pin);
                tracing::debug!("Generated PIN for session {}, peer {}", session_id, peer_id);
            } else {
                session.state = PairingState::Failed("User rejected".into());
                pin_result = None;
            }
        } else {
            return Err(DecentPasteError::Pairing("Session not found".into()));
        }
    }

    // Send the response via network (outside the lock)
    let tx = state.network_command_tx.read().await;
    if let Some(tx) = tx.as_ref() {
        if accept {
            if let Some(ref pin) = pin_result {
                // Get device identity for the challenge (includes our public key for ECDH)
                let device_identity = state.device_identity.read().await;
                let identity = device_identity
                    .as_ref()
                    .ok_or(DecentPasteError::NotInitialized)?;

                if tx
                    .send(NetworkCommand::SendPairingChallenge {
                        peer_id,
                        session_id: session_id.clone(),
                        pin: pin.clone(),
                        device_name: identity.device_name.clone(),
                        public_key: identity.public_key.clone(), // Our X25519 public key for ECDH
                    })
                    .await
                    .is_err()
                {
                    // Rollback session state on network failure
                    let mut sessions = state.pairing_sessions.write().await;
                    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id)
                    {
                        session.state = PairingState::Initiated;
                        session.pin = None;
                        tracing::warn!(
                            "Rolled back session {} after network send failure",
                            session_id
                        );
                    }
                    return Err(DecentPasteError::ChannelSend);
                }
            }
        } else {
            // For reject, failure to send is less critical - session is already marked failed
            let _ = tx
                .send(NetworkCommand::RejectPairing {
                    peer_id,
                    session_id,
                })
                .await;
        }
    }

    Ok(pin_result)
}

#[tauri::command]
pub async fn confirm_pairing(
    state: State<'_, AppState>,
    session_id: String,
    pin: String,
) -> Result<bool> {
    let peer_id: String;
    let is_initiator: bool;

    {
        let mut sessions = state.pairing_sessions.write().await;

        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            if session.pin.as_ref() != Some(&pin) {
                session.state = PairingState::Failed("Invalid PIN".into());
                return Ok(false);
            }

            peer_id = session.peer_id.clone();
            is_initiator = session.is_initiator;
            session.state = PairingState::AwaitingPeerConfirmation;
        } else {
            return Err(DecentPasteError::Pairing("Session not found".into()));
        }
    }

    if is_initiator {
        // Initiator: Derive shared secret using X25519 ECDH
        let device_identity = state.device_identity.read().await;
        let identity = device_identity
            .as_ref()
            .ok_or(DecentPasteError::NotInitialized)?;

        // Get the peer's public key from the session
        let peer_public_key = {
            let sessions = state.pairing_sessions.read().await;
            sessions
                .iter()
                .find(|s| s.session_id == session_id)
                .and_then(|s| s.peer_public_key.clone())
                .ok_or_else(|| DecentPasteError::Pairing("Peer public key not found".into()))?
        };

        // Derive shared secret using ECDH: our_private_key + their_public_key
        let our_private_key = identity
            .private_key
            .as_ref()
            .ok_or_else(|| DecentPasteError::Pairing("Private key not found".into()))?;

        let shared_secret =
            crate::security::derive_shared_secret(our_private_key, &peer_public_key)?;

        tracing::debug!(
            "Initiator derived shared secret via ECDH, sending confirm to peer {}",
            peer_id
        );

        let tx = state.network_command_tx.read().await;
        if let Some(tx) = tx.as_ref() {
            tx.send(NetworkCommand::SendPairingConfirm {
                peer_id: peer_id.clone(),
                session_id: session_id.clone(),
                success: true,
                shared_secret: Some(shared_secret), // Send for verification (responder will also derive)
                device_name: identity.device_name.clone(),
            })
            .await
            .map_err(|_| DecentPasteError::ChannelSend)?;
        }

        // Note: Don't add to paired peers yet - wait for confirmation response
        // The PairingComplete event will be emitted when we receive the ack from responder

        Ok(true)
    } else {
        // Responder: Just mark as locally confirmed.
        // The actual completion happens when we receive the PairingConfirm from the initiator
        // via the network. The NetworkManager will emit PairingComplete when that happens.
        // At that point, responder will also derive shared secret via ECDH.
        tracing::debug!("Responder confirmed PIN locally, waiting for initiator's confirmation");
        Ok(true)
    }
}

#[tauri::command]
pub async fn cancel_pairing(state: State<'_, AppState>, session_id: String) -> Result<()> {
    let mut sessions = state.pairing_sessions.write().await;
    sessions.retain(|s| s.session_id != session_id);
    Ok(())
}

// Clipboard operations
#[tauri::command]
pub async fn get_clipboard_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<ClipboardEntry>> {
    let history = state.clipboard_history.read().await;
    let limit = limit.unwrap_or(50);
    Ok(history.iter().take(limit).cloned().collect())
}

#[tauri::command]
pub async fn set_clipboard(app_handle: AppHandle, content: String) -> Result<()> {
    crate::clipboard::monitor::set_clipboard_content(&app_handle, &content)
        .map_err(DecentPasteError::Clipboard)
}

/// Manually share clipboard content with paired peers.
/// This is especially useful on mobile where automatic clipboard monitoring is not available.
#[tauri::command]
pub async fn share_clipboard_content(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    content: String,
) -> Result<()> {
    use crate::clipboard::ClipboardEntry;
    use crate::network::{ClipboardMessage, NetworkCommand};
    use crate::security::{encrypt_content, hash_content};
    use chrono::Utc;
    use tauri::Emitter;

    // Limit clipboard content size to prevent memory exhaustion (1MB max)
    const MAX_CLIPBOARD_SIZE: usize = 1024 * 1024;
    if content.len() > MAX_CLIPBOARD_SIZE {
        return Err(DecentPasteError::InvalidInput(
            "Clipboard content too large (max 1MB)".into(),
        ));
    }

    let content_hash = hash_content(&content);

    // Get device info
    let device_identity = state.device_identity.read().await;
    let identity = device_identity
        .as_ref()
        .ok_or(DecentPasteError::NotInitialized)?;

    // Check if we have any paired peers
    let paired_peers = state.paired_peers.read().await;
    if paired_peers.is_empty() {
        return Err(DecentPasteError::Pairing("No paired peers".into()));
    }

    // Encrypt and send to EACH paired peer with their specific shared secret
    let tx = state.network_command_tx.read().await;
    let mut broadcast_count = 0;

    for peer in paired_peers.iter() {
        let encrypted = encrypt_content(content.as_bytes(), &peer.shared_secret)
            .map_err(|e| DecentPasteError::Encryption(e.to_string()))?;

        let msg = ClipboardMessage {
            id: uuid::Uuid::new_v4().to_string(),
            content_hash: content_hash.clone(),
            encrypted_content: encrypted,
            timestamp: Utc::now(),
            origin_device_id: identity.device_id.clone(),
            origin_device_name: identity.device_name.clone(),
        };

        // Send via network
        if let Some(tx) = tx.as_ref() {
            tx.send(NetworkCommand::BroadcastClipboard { message: msg })
                .await
                .map_err(|_| DecentPasteError::ChannelSend)?;
            broadcast_count += 1;
        }
    }

    if broadcast_count == 0 {
        return Err(DecentPasteError::ChannelSend);
    }

    // Add to history (once, not per peer)
    let entry = ClipboardEntry::new_local(content, &identity.device_id, &identity.device_name);
    state.add_clipboard_entry(entry.clone()).await;

    // Emit to frontend
    let _ = app_handle.emit("clipboard-sent", entry);

    Ok(())
}

#[tauri::command]
pub async fn clear_clipboard_history(state: State<'_, AppState>) -> Result<()> {
    {
        let mut history = state.clipboard_history.write().await;
        history.clear();
    }
    // Flush-on-write: persist cleared history to vault immediately
    state.flush_clipboard_history().await?;
    Ok(())
}

// Settings
#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings> {
    let settings = state.settings.read().await;
    Ok(settings.clone())
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, settings: AppSettings) -> Result<()> {
    // Check if device name changed
    let old_device_name = {
        let current = state.settings.read().await;
        current.device_name.clone()
    };
    let name_changed = old_device_name != settings.device_name;

    save_settings(&settings)?;

    // Update state
    {
        let mut current = state.settings.write().await;
        *current = settings.clone();
    }

    // If device name changed, broadcast the new name to all peers
    if name_changed {
        debug!(
            "Device name changed from '{}' to '{}', broadcasting update",
            old_device_name, settings.device_name
        );

        // Update the device identity in memory
        {
            let mut identity = state.device_identity.write().await;
            if let Some(ref mut id) = *identity {
                id.device_name = settings.device_name.clone();
            }
        }

        // Flush-on-write: persist identity to vault immediately
        if let Err(e) = state.flush_device_identity().await {
            warn!("Failed to flush device identity to vault: {}", e);
        }

        // Broadcast the name change to all peers
        let tx = state.network_command_tx.read().await;
        if let Some(tx) = tx.as_ref() {
            let _ = tx
                .send(NetworkCommand::AnnounceDeviceName {
                    device_name: settings.device_name,
                })
                .await;
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_device_info(state: State<'_, AppState>) -> Result<DeviceInfo> {
    let identity = state.device_identity.read().await;

    if let Some(ref id) = *identity {
        Ok(DeviceInfo {
            device_id: id.device_id.clone(),
            peer_id: None, // Will be set from network
        })
    } else {
        Err(DecentPasteError::NotInitialized)
    }
}

// Get active pairing sessions
#[tauri::command]
pub async fn get_pairing_sessions(state: State<'_, AppState>) -> Result<Vec<PairingSession>> {
    let sessions = state.pairing_sessions.read().await;
    Ok(sessions
        .iter()
        .filter(|s| !s.is_expired())
        .cloned()
        .collect())
}

// ============================================================================
// Vault Commands - Secure storage authentication and management
// ============================================================================

use crate::vault::{VaultManager, VaultStatus};
use tauri::Emitter;

/// Get the current vault status.
///
/// Returns whether the vault is:
/// - NotSetup: First-time user, needs onboarding
/// - Locked: Vault exists but requires PIN to unlock
/// - Unlocked: Vault is open and data is accessible
#[tauri::command]
pub async fn get_vault_status(state: State<'_, AppState>) -> Result<VaultStatus> {
    // First check if vault file exists
    let vault_exists = VaultManager::exists().unwrap_or(false);

    if !vault_exists {
        // No vault file - user needs to set up
        let mut status = state.vault_status.write().await;
        *status = VaultStatus::NotSetup;
        return Ok(VaultStatus::NotSetup);
    }

    // Vault exists - check if it's unlocked
    let manager = state.vault_manager.read().await;
    if manager.as_ref().is_some_and(|m| m.is_open()) {
        Ok(VaultStatus::Unlocked)
    } else {
        Ok(VaultStatus::Locked)
    }
}

/// Set up a new vault during first-time onboarding.
///
/// This creates an encrypted Stronghold vault protected by the user's PIN.
/// The PIN is transformed via Argon2id into an encryption key.
/// After setup, network services are started automatically.
///
/// # Arguments
/// * `device_name` - The user's chosen device name
/// * `pin` - The user's chosen PIN (4-8 digits)
/// * `auth_method` - Auth method (currently only "pin" is supported)
#[tauri::command]
pub async fn setup_vault(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    device_name: String,
    pin: String,
    auth_method: String,
) -> Result<()> {
    use tracing::info;

    // Validate PIN length (4-8 digits)
    if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(DecentPasteError::InvalidInput(
            "PIN must be 4-8 digits".into(),
        ));
    }

    info!("Setting up new vault for device: {}", device_name);

    // Create the vault
    let mut manager = VaultManager::new();
    manager.create(&pin)?;

    // Create device identity with X25519 keypair for ECDH
    let identity = crate::security::generate_device_identity(&device_name);
    manager.set_device_identity(&identity)?;

    // Generate and store libp2p keypair
    let keypair = libp2p::identity::Keypair::generate_ed25519();
    manager.set_libp2p_keypair(&keypair)?;

    // Flush to ensure data is persisted
    manager.flush()?;

    // Update app state
    {
        let mut vault_manager = state.vault_manager.write().await;
        *vault_manager = Some(manager);
    }
    {
        let mut vault_status = state.vault_status.write().await;
        *vault_status = VaultStatus::Unlocked;
    }
    {
        let mut device_identity = state.device_identity.write().await;
        *device_identity = Some(identity);
    }

    // Update settings with auth method
    {
        let mut settings = state.settings.write().await;
        settings.device_name = device_name;
        settings.auth_method = Some(auth_method);
        save_settings(&settings)?;
    }

    // Emit vault status change
    let _ = app_handle.emit("vault-status", VaultStatus::Unlocked);

    // Start network and clipboard services now that vault is unlocked
    if let Err(e) = crate::start_network_services(app_handle.clone()).await {
        tracing::error!("Failed to start network services: {}", e);
        // Don't fail the vault setup - services can be started later
    }

    info!("Vault setup completed successfully");
    Ok(())
}

/// Unlock an existing vault with the user's PIN.
///
/// On success, loads all encrypted data from the vault into app state
/// and starts network/clipboard services.
#[tauri::command]
pub async fn unlock_vault(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    pin: String,
) -> Result<()> {
    use tracing::info;

    info!("Attempting to unlock vault");

    // Try to open the vault with the provided PIN
    let mut manager = VaultManager::new();
    manager.open(&pin)?;

    // Load data from vault into app state
    if let Ok(Some(identity)) = manager.get_device_identity() {
        let mut device_identity = state.device_identity.write().await;
        *device_identity = Some(identity);
    }

    if let Ok(peers) = manager.get_paired_peers() {
        let mut paired_peers = state.paired_peers.write().await;
        *paired_peers = peers;
    }

    if let Ok(history) = manager.get_clipboard_history() {
        let mut clipboard_history = state.clipboard_history.write().await;
        *clipboard_history = history;
    }

    // Update vault state
    {
        let mut vault_manager = state.vault_manager.write().await;
        *vault_manager = Some(manager);
    }
    {
        let mut vault_status = state.vault_status.write().await;
        *vault_status = VaultStatus::Unlocked;
    }

    // Emit vault status change
    let _ = app_handle.emit("vault-status", VaultStatus::Unlocked);

    // Start network and clipboard services now that vault is unlocked
    if let Err(e) = crate::start_network_services(app_handle.clone()).await {
        tracing::error!("Failed to start network services: {}", e);
        // Don't fail the unlock - services can be started later
    }

    info!("Vault unlocked successfully");
    Ok(())
}

/// Lock the vault, flushing all data and clearing keys from memory.
///
/// After locking, the user must enter their PIN to access data again.
#[tauri::command]
pub async fn lock_vault(app_handle: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tracing::info;

    info!("Locking vault");

    // Flush current state to vault before locking (safety net - data should already be persisted)
    let _ = state.flush_all_to_vault().await;

    // Lock the vault (clears encryption key from memory)
    {
        let mut manager = state.vault_manager.write().await;
        if let Some(ref mut m) = *manager {
            m.lock()?;
        }
        *manager = None;
    }

    // Update status
    {
        let mut vault_status = state.vault_status.write().await;
        *vault_status = VaultStatus::Locked;
    }

    // Emit vault status change
    let _ = app_handle.emit("vault-status", VaultStatus::Locked);

    info!("Vault locked successfully");
    Ok(())
}

/// Reset the vault, destroying all encrypted data.
///
/// This is a destructive operation that:
/// 1. Deletes the vault file
/// 2. Deletes the salt file
/// 3. Clears all app state
///
/// After reset, the user must go through onboarding again.
#[tauri::command]
pub async fn reset_vault(app_handle: AppHandle, state: State<'_, AppState>) -> Result<()> {
    use tracing::{info, warn};

    warn!("Resetting vault - all encrypted data will be lost!");

    // Destroy the vault
    {
        let mut manager = state.vault_manager.write().await;
        if let Some(ref mut m) = *manager {
            m.destroy()?;
        } else {
            // No manager in memory, create one just to destroy files
            let mut temp_manager = VaultManager::new();
            temp_manager.destroy()?;
        }
        *manager = None;
    }

    // Clear app state
    {
        let mut device_identity = state.device_identity.write().await;
        *device_identity = None;
    }
    {
        let mut paired_peers = state.paired_peers.write().await;
        paired_peers.clear();
    }
    {
        let mut clipboard_history = state.clipboard_history.write().await;
        clipboard_history.clear();
    }
    {
        let mut vault_status = state.vault_status.write().await;
        *vault_status = VaultStatus::NotSetup;
    }

    // Emit vault status change
    let _ = app_handle.emit("vault-status", VaultStatus::NotSetup);

    info!("Vault reset completed");
    Ok(())
}

/// Flush current app state to the vault.
///
/// Saves clipboard history and paired peers to the encrypted vault.
/// With flush-on-write pattern, this is mainly a safety net.
#[tauri::command]
pub async fn flush_vault(state: State<'_, AppState>) -> Result<()> {
    state.flush_all_to_vault().await
}

// ============================================================================
// Share Intent Handling - For Android "share with" functionality
// ============================================================================

/// Result of handling shared content from Android share intent.
/// This is a DTO - the UI decides how to present these values to the user.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareResult {
    /// Total number of paired peers
    pub total_peers: usize,
    /// Number of peers that were online and received the content
    pub peers_reached: usize,
    /// Number of peers that were offline
    pub peers_offline: usize,
    /// Whether the content was added to clipboard history
    pub added_to_history: bool,
}

/// Summary of connection status after ensure_connected() completes.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionSummary {
    /// Total number of paired peers
    pub total_peers: usize,
    /// Number of peers currently connected
    pub connected: usize,
    /// Number of peers that failed to connect
    pub failed: usize,
}

// =============================================================================
// Connection Management
// =============================================================================

/// Ensure all paired peers are connected.
///
/// This function:
/// 1. Guards against concurrent reconnection attempts
/// 2. Only dials peers that are currently disconnected
/// 3. Waits for all dials to complete or timeout
/// 4. Returns a summary of connection status
///
/// The function is awaitable - it returns when all dial attempts complete,
/// not fire-and-forget like the old reconnect_peers command.
pub async fn ensure_connected(state: &AppState, timeout: Duration) -> ConnectionSummary {
    // Guard: only one reconnection at a time
    if state
        .reconnect_in_progress
        .swap(true, Ordering::SeqCst)
    {
        // Already reconnecting - wait for it to finish
        let _ = tokio::time::timeout(timeout, state.dials_complete_notify.notified()).await;
        return get_connection_summary(state).await;
    }

    let paired = state.paired_peers.read().await;
    let total_peers = paired.len();

    if total_peers == 0 {
        state.reconnect_in_progress.store(false, Ordering::SeqCst);
        return ConnectionSummary {
            total_peers: 0,
            connected: 0,
            failed: 0,
        };
    }

    // Find disconnected peers (status != Connected)
    let to_dial: Vec<_> = {
        let conns = state.peer_connections.read().await;
        paired
            .iter()
            .filter(|p| {
                conns
                    .get(&p.peer_id)
                    .map(|c| c.status != ConnectionStatus::Connected)
                    .unwrap_or(true) // Not in map = disconnected
            })
            .cloned()
            .collect()
    };
    drop(paired); // Release read lock before write

    if to_dial.is_empty() {
        // All peers already connected
        state.reconnect_in_progress.store(false, Ordering::SeqCst);
        return get_connection_summary(state).await;
    }

    // Mark as Connecting and count pending dials
    {
        let mut conns = state.peer_connections.write().await;
        for peer in &to_dial {
            // Get last_connected value before mutable borrow
            let last_connected = conns.get(&peer.peer_id).and_then(|c| c.last_connected);
            conns.insert(
                peer.peer_id.clone(),
                PeerConnectionState {
                    status: ConnectionStatus::Connecting,
                    last_connected,
                },
            );
        }
    }
    state.pending_dials.store(to_dial.len(), Ordering::SeqCst);

    debug!(
        "Dialing {} disconnected peers (timeout: {:?})",
        to_dial.len(),
        timeout
    );

    // Collect addresses for reconnection
    let addresses: Vec<(String, Vec<String>)> = to_dial
        .iter()
        .filter(|p| !p.last_known_addresses.is_empty())
        .map(|p| (p.peer_id.clone(), p.last_known_addresses.clone()))
        .collect();

    // Trigger dials via network command
    if let Some(tx) = state.network_command_tx.read().await.as_ref() {
        let _ = tx
            .send(NetworkCommand::ReconnectPeers {
                paired_peer_addresses: addresses,
            })
            .await;
    }

    // Wait for all dials to complete OR timeout
    let _ = tokio::time::timeout(timeout, state.dials_complete_notify.notified()).await;

    // Mark any still-connecting peers as disconnected (timeout)
    {
        let mut conns = state.peer_connections.write().await;
        for (_, conn) in conns.iter_mut() {
            if conn.status == ConnectionStatus::Connecting {
                conn.status = ConnectionStatus::Disconnected;
            }
        }
    }

    // Reset pending count and guard
    state.pending_dials.store(0, Ordering::SeqCst);
    state.reconnect_in_progress.store(false, Ordering::SeqCst);

    get_connection_summary(state).await
}

/// Get a summary of current connection status for paired peers.
async fn get_connection_summary(state: &AppState) -> ConnectionSummary {
    let paired = state.paired_peers.read().await;
    let conns = state.peer_connections.read().await;

    let connected = paired
        .iter()
        .filter(|p| {
            conns
                .get(&p.peer_id)
                .map(|c| c.status == ConnectionStatus::Connected)
                .unwrap_or(false)
        })
        .count();

    ConnectionSummary {
        total_peers: paired.len(),
        connected,
        failed: paired.len() - connected,
    }
}

/// Handle shared content received from Android share intent.
///
/// This command is called by the frontend after receiving a "share-received" event
/// from the decentshare plugin. It:
/// 1. Verifies the vault is unlocked
/// 2. Ensures paired peers are connected (awaitable, with timeout)
/// 3. Shares the content with all connected paired peers
/// 4. Adds the content to clipboard history
/// 5. Returns honest messaging about delivery status
///
/// # Arguments
/// * `content` - The shared text content
///
/// # Returns
/// * `ShareResult` - Details about the sharing operation
#[tauri::command]
pub async fn handle_shared_content(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    content: String,
) -> Result<ShareResult> {
    use crate::vault::VaultStatus;

    info!(
        "Handling shared content from share intent ({} chars)",
        content.len()
    );

    // 1. Check vault is unlocked
    let vault_status = *state.vault_status.read().await;
    if vault_status != VaultStatus::Unlocked {
        return Err(DecentPasteError::VaultLocked);
    }

    // 2. Check we have paired peers
    let paired_count = state.paired_peers.read().await.len();
    if paired_count == 0 {
        return Err(DecentPasteError::NoPeersAvailable);
    }

    // 3. Ensure connected (awaitable, 3s timeout)
    // This dials disconnected peers and waits for connections to establish
    let summary = ensure_connected(&state, Duration::from_secs(3)).await;

    info!(
        "Connection summary: {}/{} connected",
        summary.connected, summary.total_peers
    );

    // 4. Share the content using existing share_clipboard_content logic
    // This handles encryption, broadcast, and history
    share_clipboard_content(app_handle.clone(), state.clone(), content).await?;

    // 5. Return DTO - UI decides how to present this to user
    Ok(ShareResult {
        total_peers: summary.total_peers,
        peers_reached: summary.connected,
        peers_offline: summary.failed,
        added_to_history: true,
    })
}

/// Refresh connections to all paired peers.
///
/// This is an awaitable command that can be called from the UI refresh button.
/// It triggers reconnection to disconnected peers and waits for completion.
#[tauri::command]
pub async fn refresh_connections(state: State<'_, AppState>) -> Result<ConnectionSummary> {
    info!("Manual refresh connections requested");
    Ok(ensure_connected(&state, Duration::from_secs(5)).await)
}

// Note: wait_for_peers_ready has been replaced by ensure_connected()
// which uses proper event-driven waiting instead of polling.
