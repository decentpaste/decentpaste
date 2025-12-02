use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

use crate::security::hash_content;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: String,
    pub content: String,
    pub content_hash: String,
    pub timestamp: DateTime<Utc>,
    pub origin_device_id: String,
    pub origin_device_name: String,
    pub is_local: bool,
}

impl ClipboardEntry {
    pub fn new_local(content: String, device_id: &str, device_name: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content_hash: hash_content(&content),
            content,
            timestamp: Utc::now(),
            origin_device_id: device_id.to_string(),
            origin_device_name: device_name.to_string(),
            is_local: true,
        }
    }

    pub fn new_remote(
        content: String,
        content_hash: String,
        timestamp: DateTime<Utc>,
        device_id: &str,
        device_name: &str,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            content,
            content_hash,
            timestamp,
            origin_device_id: device_id.to_string(),
            origin_device_name: device_name.to_string(),
            is_local: false,
        }
    }

    pub fn preview(&self, max_length: usize) -> String {
        if self.content.len() <= max_length {
            self.content.clone()
        } else {
            format!("{}...", &self.content[..max_length])
        }
    }
}

const RECENT_HASH_TTL_SECS: u64 = 10;
const MAX_RECENT_HASHES: usize = 100;

pub struct SyncManager {
    recent_hashes: HashMap<String, Instant>,
    local_hash: Option<String>,
}

impl SyncManager {
    pub fn new() -> Self {
        Self {
            recent_hashes: HashMap::new(),
            local_hash: None,
        }
    }

    pub fn should_broadcast(&mut self, content_hash: &str, is_local: bool) -> bool {
        // Clean up expired hashes
        self.cleanup_expired();

        // Don't broadcast if:
        // 1. Hash matches recent received content (loop prevention)
        if self.recent_hashes.contains_key(content_hash) {
            return false;
        }

        // 2. Hash matches last broadcast (no change)
        if self.local_hash.as_ref() == Some(&content_hash.to_string()) {
            return false;
        }

        // 3. Content is from received message (!is_local)
        if !is_local {
            return false;
        }

        // Update local hash
        self.local_hash = Some(content_hash.to_string());

        true
    }

    pub fn on_received(&mut self, content_hash: &str) -> bool {
        // Clean up expired hashes
        self.cleanup_expired();

        // Track hash to prevent echo
        self.recent_hashes.insert(content_hash.to_string(), Instant::now());

        // Don't apply if hash matches current clipboard
        if self.local_hash.as_ref() == Some(&content_hash.to_string()) {
            return false;
        }

        // Update local hash to prevent echo on next poll
        self.local_hash = Some(content_hash.to_string());

        true
    }

    pub fn set_local_hash(&mut self, hash: String) {
        self.local_hash = Some(hash);
    }

    fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.recent_hashes.retain(|_, time| {
            now.duration_since(*time).as_secs() < RECENT_HASH_TTL_SECS
        });

        // Also limit size
        if self.recent_hashes.len() > MAX_RECENT_HASHES {
            // Remove oldest entries
            let mut entries: Vec<_> = self.recent_hashes.drain().collect();
            entries.sort_by_key(|(_, time)| *time);
            self.recent_hashes = entries.into_iter().skip(MAX_RECENT_HASHES / 2).collect();
        }
    }
}

impl Default for SyncManager {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConflictResolution {
    UseRemote,
    KeepLocal,
}

pub fn resolve_conflict(local_timestamp: i64, remote_timestamp: i64) -> ConflictResolution {
    let time_diff = (local_timestamp - remote_timestamp).abs();

    if time_diff > 100 {
        // Clear winner by timestamp
        if remote_timestamp > local_timestamp {
            ConflictResolution::UseRemote
        } else {
            ConflictResolution::KeepLocal
        }
    } else {
        // Near-simultaneous, prefer local
        ConflictResolution::KeepLocal
    }
}
