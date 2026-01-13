use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::protocol::{ClipboardMessage, MessageHash, PairingRequest};

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

    // Readiness events (protocol-agnostic)
    // Currently triggered by gossipsub subscription, but could be any protocol
    /// A peer is now ready to receive broadcast messages
    PeerReady {
        peer_id: String,
    },
    /// A peer is no longer ready to receive broadcast messages
    PeerNotReady {
        peer_id: String,
    },

    // Pairing events
    PairingRequestReceived {
        session_id: String,
        peer_id: String,
        request: PairingRequest,
    },
    PairingPinReady {
        session_id: String,
        pin: String,
        peer_device_name: String, // Responder's device name (for initiator to display)
        peer_public_key: Vec<u8>, // Responder's X25519 public key for ECDH
    },
    PairingComplete {
        session_id: String,
        peer_id: String,
        device_name: String,
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
    #[allow(dead_code)]
    Error(String),

    // Sync events (for offline message delivery)
    /// A peer requested sync from us - we should send them our buffered hashes.
    SyncRequestReceived {
        peer_id: String,
    },
    /// A peer requested specific content by hash.
    SyncContentRequestReceived {
        peer_id: String,
        hash: String,
    },
    /// Received a list of message hashes available from a peer.
    /// Used to determine which messages we need to request.
    SyncHashListReceived {
        peer_id: String,
        hashes: Vec<MessageHash>,
    },
    /// Received full message content after requesting via ContentRequest.
    /// Contains the clipboard message to be decrypted and added to history.
    SyncContentReceived {
        peer_id: String,
        message: ClipboardMessage,
    },
}
