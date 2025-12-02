mod clipboard;
mod commands;
mod error;
mod network;
mod security;
mod state;
mod storage;

use chrono::Utc;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use clipboard::{ClipboardChange, ClipboardEntry, ClipboardMonitor, SyncManager};
use network::{NetworkCommand, NetworkEvent, NetworkManager, NetworkStatus, ClipboardMessage};
use security::get_or_create_identity;
use state::AppState;
use storage::{init_data_dir, load_paired_peers, load_settings};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "decentpaste_app=debug,libp2p=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting DecentPaste...");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::new())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize app state
            tauri::async_runtime::spawn(async move {
                if let Err(e) = initialize_app(app_handle).await {
                    error!("Failed to initialize app: {}", e);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_network_status,
            commands::start_network,
            commands::stop_network,
            commands::get_discovered_peers,
            commands::get_paired_peers,
            commands::remove_paired_peer,
            commands::initiate_pairing,
            commands::respond_to_pairing,
            commands::confirm_pairing,
            commands::cancel_pairing,
            commands::get_clipboard_history,
            commands::set_clipboard,
            commands::share_clipboard_content,
            commands::clear_clipboard_history,
            commands::get_settings,
            commands::update_settings,
            commands::get_device_info,
            commands::get_pairing_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn initialize_app(app_handle: AppHandle) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize data directory first (required for all storage operations)
    init_data_dir(&app_handle)?;

    let state = app_handle.state::<AppState>();

    // Load settings
    let settings = load_settings().unwrap_or_default();
    {
        let mut s = state.settings.write().await;
        *s = settings.clone();
    }

    // Load or create device identity
    let identity = get_or_create_identity()?;
    {
        let mut id = state.device_identity.write().await;
        *id = Some(identity.clone());
    }
    info!("Device ID: {}", identity.device_id);

    // Load paired peers
    let paired = load_paired_peers().unwrap_or_default();
    {
        let mut peers = state.paired_peers.write().await;
        *peers = paired;
    }

    // Create channels
    let (network_cmd_tx, network_cmd_rx) = mpsc::channel::<NetworkCommand>(100);
    let (network_event_tx, mut network_event_rx) = mpsc::channel::<NetworkEvent>(100);
    let (clipboard_tx, mut clipboard_rx) = mpsc::channel::<ClipboardChange>(100);

    // Store network command sender
    {
        let mut tx = state.network_command_tx.write().await;
        *tx = Some(network_cmd_tx.clone());
    }

    // Start network manager
    let network_event_tx_clone = network_event_tx.clone();
    tokio::spawn(async move {
        match NetworkManager::new(network_cmd_rx, network_event_tx_clone).await {
            Ok(mut manager) => {
                info!("Network manager started, peer ID: {}", manager.local_peer_id());
                manager.run().await;
            }
            Err(e) => {
                error!("Failed to create network manager: {}", e);
            }
        }
    });

    // Start clipboard monitor
    let clipboard_monitor = ClipboardMonitor::new(settings.clipboard_poll_interval_ms);
    clipboard_monitor.start(app_handle.clone(), clipboard_tx).await;

    // Handle clipboard changes - broadcast to network
    let app_handle_clipboard = app_handle.clone();
    let network_cmd_tx_clipboard = network_cmd_tx.clone();
    tokio::spawn(async move {
        let state = app_handle_clipboard.state::<AppState>();
        let mut sync_manager = SyncManager::new();

        while let Some(change) = clipboard_rx.recv().await {
            if change.is_local && sync_manager.should_broadcast(&change.content_hash, true) {
                // Get device info
                let device_identity = state.device_identity.read().await;
                if let Some(ref identity) = *device_identity {
                    // Check if we have any paired peers
                    let paired_peers = state.paired_peers.read().await;
                    if paired_peers.is_empty() {
                        continue;
                    }

                    // Encrypt content for each paired peer (simplified - using first peer's secret)
                    if let Some(peer) = paired_peers.first() {
                        match security::encrypt_content(
                            change.content.as_bytes(),
                            &peer.shared_secret,
                        ) {
                            Ok(encrypted) => {
                                let msg = ClipboardMessage {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    content_hash: change.content_hash.clone(),
                                    encrypted_content: encrypted,
                                    timestamp: Utc::now(),
                                    origin_device_id: identity.device_id.clone(),
                                    origin_device_name: identity.device_name.clone(),
                                };

                                let _ = network_cmd_tx_clipboard
                                    .send(NetworkCommand::BroadcastClipboard { message: msg })
                                    .await;

                                // Add to history
                                let entry = ClipboardEntry::new_local(
                                    change.content,
                                    &identity.device_id,
                                    &identity.device_name,
                                );
                                state.add_clipboard_entry(entry.clone()).await;

                                // Emit to frontend
                                let _ = app_handle_clipboard.emit("clipboard-sent", entry);
                            }
                            Err(e) => {
                                error!("Failed to encrypt clipboard content: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    // Handle network events
    let app_handle_network = app_handle.clone();
    tokio::spawn(async move {
        let state = app_handle_network.state::<AppState>();
        let mut sync_manager = SyncManager::new();

        while let Some(event) = network_event_rx.recv().await {
            match event {
                NetworkEvent::StatusChanged(status) => {
                    let mut s = state.network_status.write().await;
                    *s = status.clone();
                    let _ = app_handle_network.emit("network-status", status);
                }

                NetworkEvent::PeerDiscovered(peer) => {
                    let mut peers = state.discovered_peers.write().await;
                    if !peers.iter().any(|p| p.peer_id == peer.peer_id) {
                        peers.push(peer.clone());
                    }
                    let _ = app_handle_network.emit("peer-discovered", peer);
                }

                NetworkEvent::PeerLost(peer_id) => {
                    let mut peers = state.discovered_peers.write().await;
                    peers.retain(|p| p.peer_id != peer_id);
                    let _ = app_handle_network.emit("peer-lost", peer_id);
                }

                NetworkEvent::PeerConnected(peer) => {
                    let _ = app_handle_network.emit("peer-connected", peer);
                }

                NetworkEvent::PeerDisconnected(peer_id) => {
                    let _ = app_handle_network.emit("peer-disconnected", peer_id);
                }

                NetworkEvent::PairingRequestReceived {
                    session_id,
                    peer_id,
                    request,
                } => {
                    let session = security::PairingSession::new(
                        session_id.clone(),
                        peer_id.clone(),
                        false,
                    )
                    .with_peer_name(request.device_name.clone());

                    let mut sessions = state.pairing_sessions.write().await;
                    sessions.push(session);

                    let _ = app_handle_network.emit("pairing-request", serde_json::json!({
                        "sessionId": session_id,
                        "peerId": peer_id,
                        "deviceName": request.device_name,
                    }));
                }

                NetworkEvent::PairingPinReady { session_id, pin } => {
                    let mut sessions = state.pairing_sessions.write().await;
                    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id)
                    {
                        session.pin = Some(pin.clone());
                        session.state = security::PairingState::AwaitingPinConfirmation;
                    }
                    let _ = app_handle_network.emit("pairing-pin", serde_json::json!({
                        "sessionId": session_id,
                        "pin": pin,
                    }));
                }

                NetworkEvent::PairingComplete {
                    session_id,
                    peer_id,
                    device_name,
                    shared_secret,
                } => {
                    // Update session state
                    {
                        let mut sessions = state.pairing_sessions.write().await;
                        if let Some(session) =
                            sessions.iter_mut().find(|s| s.session_id == session_id)
                        {
                            session.state = security::PairingState::Completed;
                        }
                    }

                    // Add to paired peers
                    let paired_peer = storage::PairedPeer {
                        peer_id: peer_id.clone(),
                        device_name: device_name.clone(),
                        shared_secret,
                        paired_at: Utc::now(),
                        last_seen: Some(Utc::now()),
                    };

                    {
                        let mut peers = state.paired_peers.write().await;
                        if !peers.iter().any(|p| p.peer_id == peer_id) {
                            peers.push(paired_peer);
                            let _ = storage::save_paired_peers(&peers);
                        }
                    }

                    let _ = app_handle_network.emit("pairing-complete", serde_json::json!({
                        "sessionId": session_id,
                        "peerId": peer_id,
                        "deviceName": device_name,
                    }));
                }

                NetworkEvent::PairingFailed { session_id, error } => {
                    let mut sessions = state.pairing_sessions.write().await;
                    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id)
                    {
                        session.state = security::PairingState::Failed(error.clone());
                    }
                    let _ = app_handle_network.emit("pairing-failed", serde_json::json!({
                        "sessionId": session_id,
                        "error": error,
                    }));
                }

                NetworkEvent::ClipboardReceived(msg) => {
                    // Check if from paired peer
                    let paired_peers = state.paired_peers.read().await;

                    // Find the peer's shared secret
                    // For simplicity, try decrypting with any paired peer's secret
                    for peer in paired_peers.iter() {
                        match security::decrypt_content(&msg.encrypted_content, &peer.shared_secret)
                        {
                            Ok(decrypted) => {
                                if let Ok(content) = String::from_utf8(decrypted) {
                                    // Verify hash
                                    let hash = security::hash_content(&content);
                                    if hash == msg.content_hash {
                                        if sync_manager.on_received(&msg.content_hash) {
                                            // Update local clipboard
                                            if let Err(e) =
                                                clipboard::monitor::set_clipboard_content(&app_handle_network, &content)
                                            {
                                                error!("Failed to set clipboard: {}", e);
                                            }

                                            // Add to history
                                            let entry = ClipboardEntry::new_remote(
                                                content,
                                                msg.content_hash.clone(),
                                                msg.timestamp,
                                                &msg.origin_device_id,
                                                &msg.origin_device_name,
                                            );
                                            state.add_clipboard_entry(entry.clone()).await;

                                            // Emit to frontend
                                            let _ =
                                                app_handle_network.emit("clipboard-received", entry);
                                        }
                                        break;
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                }

                NetworkEvent::ClipboardSent { id, peer_count } => {
                    let _ = app_handle_network.emit("clipboard-broadcast", serde_json::json!({
                        "id": id,
                        "peerCount": peer_count,
                    }));
                }

                NetworkEvent::Error(error) => {
                    let _ = app_handle_network.emit("network-error", error);
                }
            }
        }
    });

    info!("DecentPaste initialized successfully");
    Ok(())
}
