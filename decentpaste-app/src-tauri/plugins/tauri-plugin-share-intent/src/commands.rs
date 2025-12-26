use tauri::command;

use crate::{Result, ShareIntentContent};

/// Get and consume pending share intent content.
///
/// This command is primarily implemented by native code (Kotlin/Swift).
/// The Rust side just provides the interface.
///
/// Returns the shared content if present, consuming it so subsequent calls return None.
#[command]
pub async fn get_pending_content() -> Result<ShareIntentContent> {
    // This will be handled by native plugin via Tauri's mobile plugin system
    // For desktop (where share intents don't exist), return empty
    Ok(ShareIntentContent {
        content: None,
        source: None,
    })
}

/// Check if there's pending share content without consuming it.
#[command]
pub async fn has_pending_content() -> Result<bool> {
    // Handled by native plugin
    Ok(false)
}

/// Clear pending content without processing.
/// Useful for cancellation flows.
#[command]
pub async fn clear_pending_content() -> Result<()> {
    // Handled by native plugin
    Ok(())
}
