//! System tray support (desktop only)
//!
//! This module provides system tray functionality for desktop platforms.
//! On mobile platforms (Android/iOS), all functions are no-ops.

/// Setup system tray with menu and click handlers (desktop only)
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn setup_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        Manager,
    };
    use tracing::info;

    // Create menu items
    let show_item = MenuItem::with_id(app, "show", "Show DecentPaste", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

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
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
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

/// No-op for mobile platforms
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn setup_tray(_app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
