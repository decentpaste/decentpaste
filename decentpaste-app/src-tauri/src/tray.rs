//! System tray support (desktop only)
//!
//! This module provides system tray functionality for desktop platforms.
//! On mobile platforms (Android/iOS), all functions are no-ops.
/// Setup system tray with menu and click handlers (desktop only)
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn setup_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use crate::state::AppState;
    use crate::storage::save_settings;
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        Emitter, Manager,
    };
    use tracing::info;

    // Get initial sync state
    let state = app.state::<AppState>();
    let sync_enabled =
        tauri::async_runtime::block_on(async { state.settings.read().await.auto_sync_enabled });

    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show DecentPaste", true, None::<&str>)?;
    let sync_label = if sync_enabled {
        "Auto Sync: On"
    } else {
        "Auto Sync: Off"
    };
    let sync_item = MenuItem::with_id(app, "sync_toggle", sync_label, true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &sync_item, &quit_item])?;

    // Keep a reference to the sync menu item for dynamic updates
    let sync_item = std::sync::Arc::new(sync_item);
    let sync_item_clone = sync_item.clone();

    // Build tray with icon from app resources
    TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("DecentPaste")
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                // Toggle window visibility on the left click
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .on_menu_event(move |app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "sync_toggle" => {
                let app = app.clone();
                let sync_item = sync_item_clone.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app.state::<AppState>();

                    // Toggle sync setting
                    let new_state = {
                        let mut settings = state.settings.write().await;
                        settings.auto_sync_enabled = !settings.auto_sync_enabled;
                        let new_state = settings.auto_sync_enabled;

                        // Save settings to disk
                        if let Err(e) = save_settings(&settings) {
                            tracing::error!("Failed to save settings: {}", e);
                        }

                        new_state
                    };

                    // Emit event to frontend to update UI
                    let _ = app.emit(
                        "settings-changed",
                        serde_json::json!({ "auto_sync_enabled": new_state }),
                    );

                    // Update the menu item text
                    let new_label = if new_state {
                        "Auto Sync: On"
                    } else {
                        "Auto Sync: Off"
                    };
                    let _ = sync_item.set_text(new_label);

                    info!("Clipboard sync toggled to: {}", new_state);
                });
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    info!("System tray initialized");
    Ok(())
}
