use tauri::{
    plugin::{Builder, TauriPlugin},
    Runtime,
};

mod commands;
mod error;

pub use error::Error;
pub type Result<T> = std::result::Result<T, Error>;

/// Share intent content received from native platform
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShareIntentContent {
    pub content: Option<String>,
    pub source: Option<String>, // "android" or "ios"
}

/// Initialize the share-intent plugin
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("share-intent")
        .invoke_handler(tauri::generate_handler![
            commands::get_pending_content,
            commands::has_pending_content,
            commands::clear_pending_content,
        ])
        .setup(|app, api| {
            // Register platform-specific plugin implementations

            #[cfg(target_os = "android")]
            {
                api.register_android_plugin(
                    "com.decentpaste.plugins.shareintent",
                    "ShareIntentPlugin",
                )?;
            }

            #[cfg(target_os = "ios")]
            {
                // iOS plugin registration happens via Swift Package
                // The plugin is automatically registered when the Swift package is linked
                let _ = (app, api); // suppress unused warnings
            }

            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let _ = (app, api); // suppress unused warnings on desktop
            }

            Ok(())
        })
        .on_event(|_app, event| {
            // Handle app lifecycle events if needed
            match event {
                tauri::RunEvent::Resumed => {
                    // App came to foreground - native plugins will check for content
                    log::info!("share-intent: App resumed");
                }
                _ => {}
            }
        })
        .build()
}
