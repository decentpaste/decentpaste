use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::peers::get_data_dir;
use crate::error::Result;

/// Application settings stored in settings.json.
///
/// Note: This struct uses `#[serde(default)]` for backward compatibility.
/// Old settings files missing new fields will use defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub device_name: String,
    pub auto_sync_enabled: bool,
    pub clipboard_history_limit: usize,
    /// Whether to persist clipboard history across app restarts.
    /// When true, history is saved to the encrypted vault.
    /// When false, history is cleared on exit.
    pub keep_history: bool,
    pub show_notifications: bool,
    pub clipboard_poll_interval_ms: u64,
    /// Preferred authentication method for vault access.
    /// Can be "pin" or "biometric". None means not yet configured (onboarding).
    pub auth_method: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            device_name: get_default_device_name(),
            auto_sync_enabled: true,
            clipboard_history_limit: 50,
            keep_history: true,
            show_notifications: true,
            clipboard_poll_interval_ms: 500,
            auth_method: None,
        }
    }
}

fn get_default_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Unknown Device".to_string())
}

fn get_settings_path() -> Result<PathBuf> {
    let data_dir = get_data_dir()?;
    Ok(data_dir.join("settings.json"))
}

pub fn load_settings() -> Result<AppSettings> {
    let path = get_settings_path()?;

    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = std::fs::read_to_string(&path)?;
    let settings: AppSettings = serde_json::from_str(&content)?;
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    let path = get_settings_path()?;
    let content = serde_json::to_string_pretty(settings)?;
    std::fs::write(&path, content)?;
    Ok(())
}
