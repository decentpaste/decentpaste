use std::sync::Arc;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, warn};

use crate::security::hash_content;

#[derive(Debug, Clone)]
pub struct ClipboardChange {
    pub content: String,
    pub content_hash: String,
    pub is_local: bool,
}

pub struct ClipboardMonitor {
    last_hash: Arc<RwLock<Option<String>>>,
    poll_interval: Duration,
    running: Arc<RwLock<bool>>,
}

impl ClipboardMonitor {
    pub fn new(poll_interval_ms: u64) -> Self {
        Self {
            last_hash: Arc::new(RwLock::new(None)),
            poll_interval: Duration::from_millis(poll_interval_ms),
            running: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn start(&self, app_handle: AppHandle, tx: mpsc::Sender<ClipboardChange>) {
        // Mark as running
        {
            let mut running = self.running.write().await;
            if *running {
                warn!("Clipboard monitor already running");
                return;
            }
            *running = true;
        }

        debug!("Starting clipboard monitor with {:?} poll interval", self.poll_interval);

        // Clone for the async task
        let last_hash = self.last_hash.clone();
        let poll_interval = self.poll_interval;
        let running = self.running.clone();

        tokio::spawn(async move {
            loop {
                // Check if we should stop
                if !*running.read().await {
                    debug!("Clipboard monitor stopping");
                    break;
                }

                // Try to read clipboard using Tauri plugin
                // Note: On Android/iOS, the Rust clipboard API may not work for reading.
                // In that case, clipboard monitoring is disabled and users share manually.
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                match app_handle.clipboard().read_text() {
                    Ok(text) => {
                        if !text.is_empty() {
                            let hash = hash_content(&text);
                            let mut last = last_hash.write().await;

                            if last.as_ref() != Some(&hash) {
                                debug!("Clipboard content changed, hash: {}", &hash[..8]);
                                *last = Some(hash.clone());

                                let change = ClipboardChange {
                                    content: text,
                                    content_hash: hash,
                                    is_local: true,
                                };

                                if tx.send(change).await.is_err() {
                                    error!("Failed to send clipboard change - receiver dropped");
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // This can happen if clipboard is empty or contains non-text
                        debug!("Could not read clipboard: {}", e);
                    }
                }

                // On mobile, clipboard monitoring from Rust is not supported
                #[cfg(any(target_os = "android", target_os = "ios"))]
                {
                    // Mobile platforms: clipboard monitoring disabled
                    // Users can manually share clipboard content via the UI
                    let _ = (&app_handle, &last_hash, &tx); // Suppress unused warnings
                }

                tokio::time::sleep(poll_interval).await;
            }

            *running.write().await = false;
        });
    }

    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    pub async fn set_last_hash(&self, hash: String) {
        let mut last = self.last_hash.write().await;
        *last = Some(hash);
    }

    pub async fn get_last_hash(&self) -> Option<String> {
        let last = self.last_hash.read().await;
        last.clone()
    }
}

pub fn set_clipboard_content(app_handle: &AppHandle, content: &str) -> Result<(), String> {
    app_handle
        .clipboard()
        .write_text(content)
        .map_err(|e| e.to_string())
}
