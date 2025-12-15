mod clipboard;
mod commands;
mod error;
mod network;
mod security;
mod state;
mod storage;
mod tray;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use clipboard::{ClipboardChange, ClipboardEntry, ClipboardMonitor, SyncManager};
use network::{ClipboardMessage, NetworkCommand, NetworkEvent, NetworkManager, NetworkStatus};
use security::get_or_create_identity;
use state::AppState;
use storage::{get_or_create_libp2p_keypair, init_data_dir, load_paired_peers, load_settings};

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
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState::new())
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Setup system tray and window close interception (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                if let Err(e) = tray::setup_tray(&app_handle) {
                    error!("Failed to setup system tray: {}", e);
                }

                // Intercept window close to hide to tray instead of quitting
                if let Some(window) = app.get_webview_window("main") {
                    let app_handle_for_close = app_handle.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            if let Some(w) = app_handle_for_close.get_webview_window("main") {
                                let _ = w.hide();
                                let _ = app_handle_for_close.emit("app-minimized-to-tray", ());
                            }
                        }
                    });
                }
            }

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
            commands::reconnect_peers,
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Handle app lifecycle events (especially for mobile)
            if let tauri::RunEvent::Resumed = event {
                info!("App resumed from background, triggering peer reconnection");
                let state = app_handle.state::<AppState>();
                let tx_arc = state.network_command_tx.clone();
                tauri::async_runtime::spawn(async move {
                    let tx = tx_arc.read().await;
                    if let Some(tx) = tx.as_ref() {
                        if let Err(e) = tx.send(NetworkCommand::ReconnectPeers).await {
                            error!("Failed to send reconnect command: {}", e);
                        }
                    }
                });
            }
        });
}

async fn initialize_app(
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

    // Load or create the libp2p keypair for consistent peer ID across restarts
    let libp2p_keypair = get_or_create_libp2p_keypair()?;
    info!("Loaded libp2p keypair, peer ID will be consistent across restarts");

    // Get device name for network identification
    let device_name = identity.device_name.clone();

    // Start network manager
    let network_event_tx_clone = network_event_tx.clone();
    tokio::spawn(async move {
        match NetworkManager::new(
            network_cmd_rx,
            network_event_tx_clone,
            libp2p_keypair,
            device_name,
        )
        .await
        {
            Ok(mut manager) => {
                info!(
                    "Network manager started, peer ID: {}",
                    manager.local_peer_id()
                );
                manager.run().await;
            }
            Err(e) => {
                error!("Failed to create network manager: {}", e);
            }
        }
    });

    // Start clipboard monitor
    let clipboard_monitor = ClipboardMonitor::new(settings.clipboard_poll_interval_ms);
    clipboard_monitor
        .start(app_handle.clone(), clipboard_tx)
        .await;

    // Create shared SyncManager for echo loop prevention
    // Both clipboard and network handlers need to share state
    let sync_manager = std::sync::Arc::new(tokio::sync::Mutex::new(SyncManager::new()));
    let sync_manager_clipboard = sync_manager.clone();
    let sync_manager_network = sync_manager.clone();

    // Handle clipboard changes - broadcast to network
    let app_handle_clipboard = app_handle.clone();
    let network_cmd_tx_clipboard = network_cmd_tx.clone();
    tokio::spawn(async move {
        let state = app_handle_clipboard.state::<AppState>();

        while let Some(change) = clipboard_rx.recv().await {
            if change.is_local
                && sync_manager_clipboard
                    .lock()
                    .await
                    .should_broadcast(&change.content_hash, true)
            {
                // Get device info
                let device_identity = state.device_identity.read().await;
                if let Some(ref identity) = *device_identity {
                    // Check if we have any paired peers
                    let paired_peers = state.paired_peers.read().await;
                    if paired_peers.is_empty() {
                        continue;
                    }

                    // Encrypt and broadcast to EACH paired peer with their specific shared secret
                    let mut broadcast_count = 0;
                    for peer in paired_peers.iter() {
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

                                if let Err(e) = network_cmd_tx_clipboard
                                    .send(NetworkCommand::BroadcastClipboard { message: msg })
                                    .await
                                {
                                    error!("Failed to send clipboard to network: {}", e);
                                } else {
                                    broadcast_count += 1;
                                }
                            }
                            Err(e) => {
                                error!(
                                    "Failed to encrypt clipboard for peer {}: {}",
                                    peer.peer_id, e
                                );
                            }
                        }
                    }

                    if broadcast_count > 0 {
                        // Add to history (once, not per peer)
                        let entry = ClipboardEntry::new_local(
                            change.content,
                            &identity.device_id,
                            &identity.device_name,
                        );
                        state.add_clipboard_entry(entry.clone()).await;

                        // Emit to frontend
                        let _ = app_handle_clipboard.emit("clipboard-sent", entry);
                    }
                }
            }
        }
    });

    // Handle network events
    let app_handle_network = app_handle.clone();
    tokio::spawn(async move {
        let state = app_handle_network.state::<AppState>();

        while let Some(event) = network_event_rx.recv().await {
            match event {
                NetworkEvent::StatusChanged(status) => {
                    let mut s = state.network_status.write().await;
                    *s = status.clone();
                    let _ = app_handle_network.emit("network-status", status);
                }

                NetworkEvent::PeerDiscovered(peer) => {
                    // Check if this peer is already paired - if so, skip adding to discovered
                    let is_paired = {
                        let paired = state.paired_peers.read().await;
                        paired.iter().any(|p| p.peer_id == peer.peer_id)
                    };

                    if !is_paired {
                        let mut peers = state.discovered_peers.write().await;
                        // Update existing peer or add new one
                        if let Some(existing) = peers.iter_mut().find(|p| p.peer_id == peer.peer_id) {
                            // Update with new info (e.g., device name from identify)
                            *existing = peer.clone();
                        } else {
                            peers.push(peer.clone());
                        }
                        let _ = app_handle_network.emit("peer-discovered", peer);
                    }
                }

                NetworkEvent::PeerLost(peer_id) => {
                    let mut peers = state.discovered_peers.write().await;
                    peers.retain(|p| p.peer_id != peer_id);
                    let _ = app_handle_network.emit("peer-lost", peer_id);
                }

                NetworkEvent::PeerNameUpdated {
                    peer_id,
                    device_name,
                } => {
                    // Update discovered peers
                    {
                        let mut peers = state.discovered_peers.write().await;
                        if let Some(peer) = peers.iter_mut().find(|p| p.peer_id == peer_id) {
                            peer.device_name = Some(device_name.clone());
                        }
                    }

                    // Update paired peers and save if changed
                    let updated_paired = {
                        let mut peers = state.paired_peers.write().await;
                        if let Some(peer) = peers.iter_mut().find(|p| p.peer_id == peer_id) {
                            if peer.device_name != device_name {
                                peer.device_name = device_name.clone();
                                // Save updated paired peers
                                let _ = storage::save_paired_peers(&peers);
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };

                    // Emit event for frontend to update
                    let _ = app_handle_network.emit(
                        "peer-name-updated",
                        serde_json::json!({
                            "peerId": peer_id,
                            "deviceName": device_name,
                        }),
                    );

                    if updated_paired {
                        info!("Updated paired peer {} name to '{}'", peer_id, device_name);
                    }
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
                    // Store the initiator's public key for ECDH key derivation later
                    let session =
                        security::PairingSession::new(session_id.clone(), peer_id.clone(), false)
                            .with_peer_name(request.device_name.clone())
                            .with_peer_public_key(request.public_key.clone());

                    let mut sessions = state.pairing_sessions.write().await;
                    // Clean up expired sessions before adding a new one
                    sessions.retain(|s| !s.is_expired());
                    sessions.push(session);

                    let _ = app_handle_network.emit(
                        "pairing-request",
                        serde_json::json!({
                            "sessionId": session_id,
                            "peerId": peer_id,
                            "deviceName": request.device_name,
                        }),
                    );
                }

                NetworkEvent::PairingPinReady {
                    session_id,
                    pin,
                    peer_device_name,
                    peer_public_key,
                } => {
                    let mut sessions = state.pairing_sessions.write().await;
                    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id)
                    {
                        session.pin = Some(pin.clone());
                        session.peer_name = Some(peer_device_name.clone());
                        session.peer_public_key = Some(peer_public_key); // Store for ECDH
                        session.state = security::PairingState::AwaitingPinConfirmation;
                    }
                    let _ = app_handle_network.emit(
                        "pairing-pin",
                        serde_json::json!({
                            "sessionId": session_id,
                            "pin": pin,
                            "peerDeviceName": peer_device_name,
                        }),
                    );
                }

                NetworkEvent::PairingComplete {
                    session_id,
                    peer_id,
                    device_name,
                    shared_secret: received_secret,
                } => {
                    // Get the device name and peer's public key from the session
                    let final_device_name: String;
                    let peer_public_key: Option<Vec<u8>>;
                    let is_responder: bool;
                    {
                        let mut sessions = state.pairing_sessions.write().await;
                        if let Some(session) =
                            sessions.iter_mut().find(|s| s.session_id == session_id)
                        {
                            session.state = security::PairingState::Completed;
                            // Use the peer_name from session if available
                            final_device_name = session.peer_name.clone().unwrap_or_else(|| {
                                if device_name == "Unknown" {
                                    "Unknown Device".to_string()
                                } else {
                                    device_name.clone()
                                }
                            });
                            peer_public_key = session.peer_public_key.clone();
                            is_responder = !session.is_initiator;
                        } else {
                            final_device_name = device_name.clone();
                            peer_public_key = None;
                            is_responder = false;
                        }
                    }

                    // Derive shared secret using ECDH if we're the responder
                    // (Initiator already derived and sent it; responder derives independently)
                    let shared_secret = if is_responder {
                        if let Some(peer_pubkey) = peer_public_key {
                            let device_identity = state.device_identity.read().await;
                            if let Some(ref identity) = *device_identity {
                                if let Some(ref our_private_key) = identity.private_key {
                                    match security::derive_shared_secret(
                                        our_private_key,
                                        &peer_pubkey,
                                    ) {
                                        Ok(derived) => {
                                            // Verify it matches what initiator sent
                                            if derived != received_secret {
                                                error!("ECDH verification failed: derived secret doesn't match received - possible MITM attack");
                                                // Fail the pairing - this is a security issue
                                                let mut sessions =
                                                    state.pairing_sessions.write().await;
                                                if let Some(session) = sessions
                                                    .iter_mut()
                                                    .find(|s| s.session_id == session_id)
                                                {
                                                    session.state = security::PairingState::Failed(
                                                        "Key verification failed".into(),
                                                    );
                                                }
                                                let _ = app_handle_network.emit(
                                                    "pairing-failed",
                                                    serde_json::json!({
                                                        "sessionId": session_id,
                                                        "error": "Key verification failed - secrets don't match",
                                                    }),
                                                );
                                                continue; // Skip adding to paired peers
                                            }
                                            derived
                                        }
                                        Err(e) => {
                                            error!("Failed to derive shared secret: {}", e);
                                            let _ = app_handle_network.emit(
                                                "pairing-failed",
                                                serde_json::json!({
                                                    "sessionId": session_id,
                                                    "error": "Failed to derive shared secret",
                                                }),
                                            );
                                            continue;
                                        }
                                    }
                                } else {
                                    error!("No private key available for ECDH derivation");
                                    let _ = app_handle_network.emit(
                                        "pairing-failed",
                                        serde_json::json!({
                                            "sessionId": session_id,
                                            "error": "Device identity incomplete",
                                        }),
                                    );
                                    continue;
                                }
                            } else {
                                error!("No device identity for ECDH derivation");
                                let _ = app_handle_network.emit(
                                    "pairing-failed",
                                    serde_json::json!({
                                        "sessionId": session_id,
                                        "error": "Device identity not found",
                                    }),
                                );
                                continue;
                            }
                        } else {
                            error!("No peer public key for ECDH derivation");
                            let _ = app_handle_network.emit(
                                "pairing-failed",
                                serde_json::json!({
                                    "sessionId": session_id,
                                    "error": "Peer public key missing",
                                }),
                            );
                            continue;
                        }
                    } else {
                        // Initiator already derived the secret
                        received_secret
                    };

                    // Add to paired peers (with duplicate check)
                    let paired_peer = storage::PairedPeer {
                        peer_id: peer_id.clone(),
                        device_name: final_device_name.clone(),
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

                    // Remove from discovered peers since they're now paired
                    {
                        let mut discovered = state.discovered_peers.write().await;
                        discovered.retain(|p| p.peer_id != peer_id);
                    }

                    let _ = app_handle_network.emit(
                        "pairing-complete",
                        serde_json::json!({
                            "sessionId": session_id,
                            "peerId": peer_id,
                            "deviceName": final_device_name,
                        }),
                    );
                }

                NetworkEvent::PairingFailed { session_id, error } => {
                    let mut sessions = state.pairing_sessions.write().await;
                    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id)
                    {
                        session.state = security::PairingState::Failed(error.clone());
                    }
                    let _ = app_handle_network.emit(
                        "pairing-failed",
                        serde_json::json!({
                            "sessionId": session_id,
                            "error": error,
                        }),
                    );
                }

                NetworkEvent::ClipboardReceived(msg) => {
                    // Check if from paired peer
                    let paired_peers = state.paired_peers.read().await;

                    // Find the peer's shared secret
                    // Try decrypting with each paired peer's secret until one succeeds
                    let mut decrypted_successfully = false;
                    for peer in paired_peers.iter() {
                        match security::decrypt_content(&msg.encrypted_content, &peer.shared_secret)
                        {
                            Ok(decrypted) => {
                                if let Ok(content) = String::from_utf8(decrypted) {
                                    // Verify hash
                                    let hash = security::hash_content(&content);
                                    if hash == msg.content_hash {
                                        decrypted_successfully = true;
                                        if sync_manager_network
                                            .lock()
                                            .await
                                            .on_received(&msg.content_hash)
                                        {
                                            // Update local clipboard
                                            if let Err(e) =
                                                clipboard::monitor::set_clipboard_content(
                                                    &app_handle_network,
                                                    &content,
                                                )
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
                                            let _ = app_handle_network
                                                .emit("clipboard-received", entry);
                                        }
                                        break;
                                    }
                                }
                            }
                            Err(_) => continue,
                        }
                    }

                    if !decrypted_successfully && !paired_peers.is_empty() {
                        tracing::warn!(
                            "Failed to decrypt clipboard message from {} - no paired peer could decrypt it",
                            msg.origin_device_name
                        );
                    }
                }

                NetworkEvent::ClipboardSent { id, peer_count } => {
                    let _ = app_handle_network.emit(
                        "clipboard-broadcast",
                        serde_json::json!({
                            "id": id,
                            "peerCount": peer_count,
                        }),
                    );
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
