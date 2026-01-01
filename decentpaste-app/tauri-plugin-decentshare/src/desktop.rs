use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

/// Initialize the desktop plugin (no-op, share intents are mobile-only).
pub fn init<R: Runtime, C: DeserializeOwned>(
    app: &AppHandle<R>,
    _api: PluginApi<R, C>,
) -> crate::Result<Decentshare<R>> {
    Ok(Decentshare(app.clone()))
}

/// Access to the decentshare APIs (desktop stub).
///
/// On desktop, share intents don't exist, so these methods return empty results.
/// The plugin still needs to be loadable on desktop for cross-platform compatibility.
pub struct Decentshare<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Decentshare<R> {
    /// Check if there's pending shared content (always returns no pending on desktop).
    pub fn get_pending_share(&self) -> crate::Result<PendingShareResponse> {
        Ok(PendingShareResponse {
            content: None,
            has_pending: false,
        })
    }

    /// Clear pending shared content (no-op on desktop).
    pub fn clear_pending_share(&self) -> crate::Result<()> {
        Ok(())
    }
}
