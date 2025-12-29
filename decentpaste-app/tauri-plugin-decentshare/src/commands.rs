use tauri::{command, AppHandle, Runtime};

use crate::models::*;
use crate::DecentshareExt;
use crate::Result;

/// Check if there's pending shared content from an Android share intent.
///
/// This should be called by the frontend after initialization to handle
/// content that was shared before the webview was ready.
#[command]
pub(crate) async fn get_pending_share<R: Runtime>(
    app: AppHandle<R>,
) -> Result<PendingShareResponse> {
    app.decentshare().get_pending_share()
}

/// Clear the pending shared content after it's been processed.
///
/// This should be called after the shared content has been successfully
/// handled to prevent it from being processed again.
#[command]
pub(crate) async fn clear_pending_share<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    app.decentshare().clear_pending_share()
}
