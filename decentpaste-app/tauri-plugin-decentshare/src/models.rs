use serde::{Deserialize, Serialize};

/// Response from the getPendingShare command.
///
/// This is returned when the frontend checks for pending shared content
/// that may have arrived via an Android share intent before the webview was ready.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingShareResponse {
    /// The shared text content, if any
    pub content: Option<String>,
    /// Whether there was pending content
    pub has_pending: bool,
}

/// Payload emitted with the "share-received" event from the mobile plugin.
///
/// This is sent to the frontend when a share intent is received.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareReceivedPayload {
    /// The shared text content
    pub content: String,
    /// Timestamp when the share was received (milliseconds since epoch)
    pub timestamp: i64,
}
