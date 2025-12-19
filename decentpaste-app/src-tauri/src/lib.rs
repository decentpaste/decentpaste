mod clipboard;
mod commands;
mod error;
mod network;
mod security;
mod state;
mod storage;
mod tray;
pub mod vault;

use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use clipboard::{ClipboardChange, ClipboardEntry, ClipboardMonitor, SyncManager};
use network::{ClipboardMessage, NetworkCommand, NetworkEvent, NetworkManager};
use state::{AppState, PendingClipboard};
use storage::{init_data_dir, load_settings};
use vault::{VaultManager, VaultStatus};

/// Track whether network services have been started (to prevent double-start)
static SERVICES_STARTED: AtomicBool = AtomicBool::new(false);

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

    // Reduce Stronghold's encryption work factor since we already use Argon2id
    // for key derivation (64MB memory, 3 iterations). The default work factor
    // causes ~35 second delays per save operation, which is unnecessary
    // when the encryption key is already cryptographically strong.
    if let Err(e) = iota_stronghold::engine::snapshot::try_set_encrypt_work_factor(0) {
        warn!("Failed to set Stronghold work factor: {:?}", e);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Stronghold plugin - password callback returns bytes for encryption key
        // Note: We handle our own Argon2 key derivation in VaultManager,
        // so this callback is mainly for the JS API compatibility
        .plugin(tauri_plugin_stronghold::Builder::new(|password| {
            password.as_bytes().to_vec()
        }).build())
        .manage(AppState::new())
        .setup(|app| {
            // Biometric plugin - only available on mobile platforms (Android/iOS)
            #[cfg(mobile)]
            app.handle().plugin(tauri_plugin_biometric::init())?;

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
            commands::process_pending_clipboard,
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
            // Vault commands
            commands::get_vault_status,
            commands::setup_vault,
            commands::unlock_vault,
            commands::lock_vault,
            commands::reset_vault,
            commands::check_biometric_available,
            commands::authenticate_biometric,
            commands::flush_vault,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Handle app lifecycle events (especially for mobile)
            match event {
                tauri::RunEvent::Resumed => {
                    info!("App resumed from background (RunEvent::Resumed)");
                    let state = app_handle.state::<AppState>();
                    let app_handle_clone = app_handle.clone();

                    // Mark as foreground
                    let is_foreground = state.is_foreground.clone();
                    let tx_arc = state.network_command_tx.clone();
                    let pending_clipboard = state.pending_clipboard.clone();

                    tauri::async_runtime::spawn(async move {
                        info!("Resume async task started");

                        // Set foreground state
                        {
                            let mut fg = is_foreground.write().await;
                            *fg = true;
                            info!("Foreground state set to true");
                        }

                        // Reconnect to peers
                        let tx = tx_arc.read().await;
                        if let Some(tx) = tx.as_ref() {
                            if let Err(e) = tx.send(NetworkCommand::ReconnectPeers).await {
                                error!("Failed to send reconnect command: {}", e);
                            }
                        }

                        // Process pending clipboard (mobile background sync)
                        #[cfg(any(target_os = "android", target_os = "ios"))]
                        {
                            info!("Checking for pending clipboard...");
                            let pending = {
                                let mut p = pending_clipboard.write().await;
                                let has_pending = p.is_some();
                                info!("Pending clipboard present: {}", has_pending);
                                p.take()
                            };
                            if let Some(pending) = pending {
                                info!(
                                    "Processing pending clipboard from {} ({} chars)",
                                    pending.from_device,
                                    pending.content.len()
                                );
                                if let Err(e) = clipboard::monitor::set_clipboard_content(
                                    &app_handle_clone,
                                    &pending.content,
                                ) {
                                    error!("Failed to set pending clipboard: {}", e);
                                } else {
                                    info!("Pending clipboard copied successfully");
                                    // Notify frontend
                                    let _ = app_handle_clone.emit(
                                        "clipboard-synced-from-background",
                                        serde_json::json!({
                                            "content": pending.content,
                                            "fromDevice": pending.from_device,
                                        }),
                                    );
                                }
                            } else {
                                info!("No pending clipboard to process");
                            }
                        }

                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        {
                            let _ = pending_clipboard; // Suppress unused warning
                            let _ = app_handle_clone;
                        }
                    });
                }

                #[cfg(any(target_os = "android", target_os = "ios"))]
                tauri::RunEvent::WindowEvent {
                    event: tauri::WindowEvent::Focused(false),
                    ..
                } => {
                    info!("App lost focus (going to background) - flushing vault");
                    let state = app_handle.state::<AppState>();
                    let is_foreground = state.is_foreground.clone();
                    let vault_manager = state.vault_manager.clone();
                    let settings = state.settings.clone();
                    let clipboard_history = state.clipboard_history.clone();
                    let paired_peers = state.paired_peers.clone();

                    tauri::async_runtime::spawn(async move {
                        // Mark as background
                        {
                            let mut fg = is_foreground.write().await;
                            *fg = false;
                        }

                        // Flush vault data before going to background
                        let manager = vault_manager.read().await;
                        if let Some(ref manager) = *manager {
                            // Check if we should save clipboard history
                            let keep_history = settings.read().await.keep_history;
                            if keep_history {
                                let history = clipboard_history.read().await;
                                if let Err(e) = manager.set_clipboard_history(&history) {
                                    error!("Failed to flush clipboard history: {}", e);
                                }
                            }

                            // Always save paired peers
                            let peers = paired_peers.read().await;
                            if let Err(e) = manager.set_paired_peers(&peers) {
                                error!("Failed to flush paired peers: {}", e);
                            }

                            // Commit to disk
                            if let Err(e) = manager.flush() {
                                error!("Failed to flush vault to disk: {}", e);
                            } else {
                                info!("Vault flushed before backgrounding");
                            }
                        }
                    });
                }

                // Flush vault on app exit (desktop)
                tauri::RunEvent::ExitRequested { .. } => {
                    info!("App exit requested - flushing vault");
                    let state = app_handle.state::<AppState>();

                    // Use blocking runtime for synchronous shutdown
                    let vault_manager = state.vault_manager.clone();
                    let settings = state.settings.clone();
                    let clipboard_history = state.clipboard_history.clone();
                    let paired_peers = state.paired_peers.clone();

                    // Spawn blocking task to ensure flush completes before exit
                    std::thread::spawn(move || {
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        rt.block_on(async {
                            let manager = vault_manager.read().await;
                            if let Some(ref manager) = *manager {
                                // Check if we should save clipboard history
                                let keep_history = settings.read().await.keep_history;
                                if keep_history {
                                    let history = clipboard_history.read().await;
                                    if let Err(e) = manager.set_clipboard_history(&history) {
                                        error!("Failed to flush clipboard history on exit: {}", e);
                                    }
                                }

                                // Always save paired peers
                                let peers = paired_peers.read().await;
                                if let Err(e) = manager.set_paired_peers(&peers) {
                                    error!("Failed to flush paired peers on exit: {}", e);
                                }

                                // Commit to disk
                                if let Err(e) = manager.flush() {
                                    error!("Failed to flush vault on exit: {}", e);
                                } else {
                                    info!("Vault flushed before exit");
                                }
                            }
                        });
                    })
                    .join()
                    .ok();
                }

                _ => {}
            }
        });
}

/// Initialize the app - check vault status and load settings.
/// Network/clipboard services are NOT started here - they start after vault unlock.
async fn initialize_app(
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize data directory first (required for all storage operations)
    init_data_dir(&app_handle)?;

    let state = app_handle.state::<AppState>();

    // Load settings (always available - not sensitive)
    let settings = load_settings().unwrap_or_default();
    {
        let mut s = state.settings.write().await;
        *s = settings.clone();
    }

    // Check vault status
    let vault_exists = VaultManager::exists().unwrap_or(false);
    let vault_status = if vault_exists {
        VaultStatus::Locked
    } else {
        VaultStatus::NotSetup
    };

    // Update state and emit event
    {
        let mut status = state.vault_status.write().await;
        *status = vault_status.clone();
    }
    let _ = app_handle.emit("vault-status", &vault_status);

    info!(
        "Vault status: {:?} - waiting for authentication before starting services",
        vault_status
    );

    // Don't start network/clipboard services here.
    // They will be started after vault is unlocked via start_network_services().

    info!("DecentPaste initialized - vault authentication required");
    Ok(())
}

/// Start network and clipboard services after vault is unlocked.
/// This is called from unlock_vault/setup_vault commands.
pub async fn start_network_services(
    app_handle: AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Prevent double-start
    if SERVICES_STARTED.swap(true, Ordering::SeqCst) {
        warn!("Network services already started, skipping");
        return Ok(());
    }

    let state = app_handle.state::<AppState>();

    // Get device identity from state (loaded from vault during unlock)
    let identity = {
        let id = state.device_identity.read().await;
        id.clone().ok_or("Device identity not loaded")?
    };
    info!("Starting network services for device: {}", identity.device_id);

    // Get settings for clipboard poll interval
    let settings = state.settings.read().await.clone();

    // Get libp2p keypair from vault manager
    let libp2p_keypair = {
        let manager = state.vault_manager.read().await;
        manager
            .as_ref()
            .ok_or("Vault not unlocked")?
            .get_libp2p_keypair()?
            .ok_or("libp2p keypair not found in vault")?
    };
    info!("Loaded libp2p keypair from vault");

    // Create channels
    let (network_cmd_tx, network_cmd_rx) = mpsc::channel::<NetworkCommand>(100);
    let (network_event_tx, mut network_event_rx) = mpsc::channel::<NetworkEvent>(100);
    let (clipboard_tx, mut clipboard_rx) = mpsc::channel::<ClipboardChange>(100);

    // Store network command sender
    {
        let mut tx = state.network_command_tx.write().await;
        *tx = Some(network_cmd_tx.clone());
    }

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
                                // Save updated paired peers to vault
                                let vault_manager = state.vault_manager.read().await;
                                if let Some(ref manager) = *vault_manager {
                                    let _ = manager.set_paired_peers(&peers);
                                    let _ = manager.flush();
                                }
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

                    // Show notification for pairing request when app is backgrounded (mobile)
                    // This is essential - pairing has a timeout and requires user action
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    {
                        let is_foreground = *state.is_foreground.read().await;
                        if !is_foreground {
                            use tauri_plugin_notification::NotificationExt;
                            info!(
                                "App in background, showing notification for pairing request from {}",
                                request.device_name
                            );
                            if let Err(e) = app_handle_network
                                .notification()
                                .builder()
                                .title("Pairing Request")
                                .body(&format!(
                                    "{} wants to pair with this device",
                                    request.device_name
                                ))
                                .show()
                            {
                                error!("Failed to show pairing notification: {}", e);
                            }
                        }
                    }

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
                            // Save to vault instead of plaintext
                            let vault_manager = state.vault_manager.read().await;
                            if let Some(ref manager) = *vault_manager {
                                let _ = manager.set_paired_peers(&peers);
                                let _ = manager.flush();
                            }
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
                                            // Check if we should queue for background (mobile only)
                                            #[cfg(any(target_os = "android", target_os = "ios"))]
                                            let is_foreground =
                                                *state.is_foreground.read().await;
                                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                            let is_foreground = true;

                                            if is_foreground {
                                                // Update local clipboard directly
                                                if let Err(e) =
                                                    clipboard::monitor::set_clipboard_content(
                                                        &app_handle_network,
                                                        &content,
                                                    )
                                                {
                                                    error!("Failed to set clipboard: {}", e);
                                                }
                                            } else {
                                                // Mobile background: queue clipboard silently (no notification)
                                                // Clipboard will be copied when app resumes
                                                #[cfg(any(target_os = "android", target_os = "ios"))]
                                                {
                                                    info!(
                                                        "App in background, queuing clipboard from {} (silent)",
                                                        msg.origin_device_name
                                                    );

                                                    // Store pending clipboard - will be processed on resume
                                                    {
                                                        let mut pending =
                                                            state.pending_clipboard.write().await;
                                                        *pending = Some(PendingClipboard {
                                                            content: content.clone(),
                                                            from_device: msg
                                                                .origin_device_name
                                                                .clone(),
                                                        });
                                                    }
                                                }
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

    info!("Network services started successfully");
    Ok(())
}
