use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::mpsc;

use crate::clipboard::{ClipboardEntry, SyncManager};
use crate::error::{DecentPasteError, Result};
use crate::network::{DiscoveredPeer, NetworkCommand, NetworkStatus};
use crate::security::{generate_pin, PairingSession, PairingState};
use crate::state::AppState;
use crate::storage::{save_settings, AppSettings, PairedPeer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub device_name: String,
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

/// Force reconnection to all discovered peers.
/// Call this when the app resumes from background on mobile.
#[tauri::command]
pub async fn reconnect_peers(state: State<'_, AppState>) -> Result<()> {
    let tx = state.network_command_tx.read().await;
    if let Some(tx) = tx.as_ref() {
        tx.send(NetworkCommand::ReconnectPeers)
            .await
            .map_err(|_| DecentPasteError::ChannelSend)?;
    }
    Ok(())
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
pub async fn remove_paired_peer(state: State<'_, AppState>, peer_id: String) -> Result<()> {
    let mut peers = state.paired_peers.write().await;
    peers.retain(|p| p.peer_id != peer_id);

    // Save to storage
    crate::storage::save_paired_peers(&peers)?;

    Ok(())
}

// Pairing flow
#[tauri::command]
pub async fn initiate_pairing(state: State<'_, AppState>, peer_id: String) -> Result<String> {
    // Check if already paired
    if state.is_peer_paired(&peer_id).await {
        return Err(DecentPasteError::AlreadyPaired(peer_id));
    }

    // Create pairing session
    let session_id = uuid::Uuid::new_v4().to_string();
    let session = PairingSession::new(session_id.clone(), peer_id.clone(), true);

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
                    .ok_or_else(|| DecentPasteError::NotInitialized)?;

                if tx
                    .send(NetworkCommand::SendPairingChallenge {
                        peer_id,
                        session_id: session_id.clone(),
                        pin: pin.clone(),
                        device_name: identity.device_name.clone(),
                        public_key: identity.public_key.clone(),  // Our X25519 public key for ECDH
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
            .ok_or_else(|| DecentPasteError::NotInitialized)?;

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

        let shared_secret = crate::security::derive_shared_secret(our_private_key, &peer_public_key)?;

        tracing::debug!("Initiator derived shared secret via ECDH, sending confirm to peer {}", peer_id);

        let tx = state.network_command_tx.read().await;
        if let Some(tx) = tx.as_ref() {
            tx.send(NetworkCommand::SendPairingConfirm {
                peer_id: peer_id.clone(),
                session_id: session_id.clone(),
                success: true,
                shared_secret: Some(shared_secret),  // Send for verification (responder will also derive)
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
        .map_err(|e| DecentPasteError::Clipboard(e))
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

    let content_hash = hash_content(&content);

    // Get device info
    let device_identity = state.device_identity.read().await;
    let identity = device_identity
        .as_ref()
        .ok_or_else(|| DecentPasteError::NotInitialized)?;

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
    let mut history = state.clipboard_history.write().await;
    history.clear();
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
    save_settings(&settings)?;
    let mut current = state.settings.write().await;
    *current = settings;
    Ok(())
}

#[tauri::command]
pub async fn get_device_info(state: State<'_, AppState>) -> Result<DeviceInfo> {
    let identity = state.device_identity.read().await;

    if let Some(ref id) = *identity {
        Ok(DeviceInfo {
            device_id: id.device_id.clone(),
            device_name: id.device_name.clone(),
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
