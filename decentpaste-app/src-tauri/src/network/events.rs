use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::protocol::{ClipboardMessage, PairingRequest};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NetworkStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeer {
    pub peer_id: String,
    pub device_name: Option<String>,
    pub addresses: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    pub is_paired: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedPeer {
    pub peer_id: String,
    pub device_name: String,
    pub connected_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    // Discovery events
    PeerDiscovered(DiscoveredPeer),
    PeerLost(String), // peer_id
    /// A peer's device name was updated (via DeviceAnnounce message)
    PeerNameUpdated {
        peer_id: String,
        device_name: String,
    },

    // Connection events
    PeerConnected(ConnectedPeer),
    PeerDisconnected(String), // peer_id

    // Pairing events
    PairingRequestReceived {
        session_id: String,
        peer_id: String,
        request: PairingRequest,
    },
    PairingPinReady {
        session_id: String,
        pin: String,
        peer_device_name: String,    // Responder's device name (for initiator to display)
        peer_public_key: Vec<u8>,    // Responder's X25519 public key for ECDH
    },
    PairingComplete {
        session_id: String,
        peer_id: String,
        device_name: String,
        shared_secret: Vec<u8>,
    },
    PairingFailed {
        session_id: String,
        error: String,
    },

    // Clipboard events
    ClipboardReceived(ClipboardMessage),
    ClipboardSent {
        id: String,
        peer_count: usize,
    },

    // Status events
    StatusChanged(NetworkStatus),
    Error(String),
}
