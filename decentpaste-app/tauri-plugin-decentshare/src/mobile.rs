use serde::de::DeserializeOwned;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::models::*;

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_decentshare);

/// Initialize the mobile plugin by registering with the native layer.
pub fn init<R: Runtime, C: DeserializeOwned>(
    _app: &AppHandle<R>,
    api: PluginApi<R, C>,
) -> crate::Result<Decentshare<R>> {
    #[cfg(target_os = "android")]
    let handle = api.register_android_plugin("com.decentpaste.plugins.decentshare", "DecentsharePlugin")?;
    #[cfg(target_os = "ios")]
    let handle = api.register_ios_plugin(init_plugin_decentshare)?;
    Ok(Decentshare(handle))
}

/// Access to the decentshare mobile APIs.
pub struct Decentshare<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Decentshare<R> {
    /// Check if there's pending shared content from an Android share intent.
    ///
    /// This handles the race condition where:
    /// 1. User shares text to DecentPaste
    /// 2. App opens but frontend isn't ready yet
    /// 3. Intent is stored in plugin's pendingShareContent
    /// 4. Frontend calls this after initialization to retrieve it
    pub fn get_pending_share(&self) -> crate::Result<PendingShareResponse> {
        self.0
            .run_mobile_plugin("getPendingShare", ())
            .map_err(Into::into)
    }

    /// Clear the pending shared content after it's been processed.
    pub fn clear_pending_share(&self) -> crate::Result<()> {
        // Kotlin returns JSObject (empty map `{}`), so we deserialize to Value and discard it
        self.0
            .run_mobile_plugin::<serde_json::Value>("clearPendingShare", ())
            .map(|_| ()) // Discard the response and return ()
            .map_err(Into::into)
    }
}
