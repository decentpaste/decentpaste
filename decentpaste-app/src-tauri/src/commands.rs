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

// Peer management
#[tauri::command]
pub async fn get_discovered_peers(state: State<'_, AppState>) -> Result<Vec<DiscoveredPeer>> {
    let peers = state.discovered_peers.read().await;
    Ok(peers.clone())
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
pub async fn initiate_pairing(
    state: State<'_, AppState>,
    peer_id: String,
) -> Result<String> {
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
    let mut sessions = state.pairing_sessions.write().await;

    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
        if accept {
            // Generate PIN
            let pin = generate_pin();
            session.pin = Some(pin.clone());
            session.state = PairingState::AwaitingPinConfirmation;
            Ok(Some(pin))
        } else {
            session.state = PairingState::Failed("User rejected".into());
            Ok(None)
        }
    } else {
        Err(DecentPasteError::Pairing("Session not found".into()))
    }
}

#[tauri::command]
pub async fn confirm_pairing(
    state: State<'_, AppState>,
    session_id: String,
    pin: String,
) -> Result<bool> {
    let mut sessions = state.pairing_sessions.write().await;

    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
        if session.pin.as_ref() == Some(&pin) {
            session.state = PairingState::Completed;

            // Generate shared secret and store pairing
            let shared_secret = crate::security::generate_shared_secret();

            let paired_peer = PairedPeer {
                peer_id: session.peer_id.clone(),
                device_name: session.peer_name.clone().unwrap_or_else(|| "Unknown Device".into()),
                shared_secret: shared_secret.clone(),
                paired_at: chrono::Utc::now(),
                last_seen: Some(chrono::Utc::now()),
            };

            // Add to paired peers
            let mut peers = state.paired_peers.write().await;
            peers.push(paired_peer);
            crate::storage::save_paired_peers(&peers)?;

            Ok(true)
        } else {
            session.state = PairingState::Failed("Invalid PIN".into());
            Ok(false)
        }
    } else {
        Err(DecentPasteError::Pairing("Session not found".into()))
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
pub async fn set_clipboard(content: String) -> Result<()> {
    crate::clipboard::monitor::set_clipboard_content(&content)
        .map_err(|e| DecentPasteError::Clipboard(e))
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
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<()> {
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
