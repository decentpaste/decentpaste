use chrono::{DateTime, Utc};
use rand::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PairingState {
    Initiated,
    AwaitingPinConfirmation,
    AwaitingPeerConfirmation,
    Completed,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingSession {
    pub session_id: String,
    pub peer_id: String,
    pub peer_name: Option<String>,
    pub peer_public_key: Option<Vec<u8>>, // Peer's X25519 public key for ECDH
    pub pin: Option<String>,
    pub state: PairingState,
    pub is_initiator: bool,
    pub created_at: DateTime<Utc>,
}

impl PairingSession {
    pub fn new(session_id: String, peer_id: String, is_initiator: bool) -> Self {
        Self {
            session_id,
            peer_id,
            peer_name: None,
            peer_public_key: None,
            pin: None,
            state: PairingState::Initiated,
            is_initiator,
            created_at: Utc::now(),
        }
    }

    pub fn with_pin(mut self, pin: String) -> Self {
        self.pin = Some(pin);
        self
    }

    pub fn with_peer_name(mut self, name: String) -> Self {
        self.peer_name = Some(name);
        self
    }

    pub fn with_peer_public_key(mut self, public_key: Vec<u8>) -> Self {
        self.peer_public_key = Some(public_key);
        self
    }

    pub fn is_expired(&self) -> bool {
        let duration = Utc::now().signed_duration_since(self.created_at);
        duration.num_minutes() > 5 // 5 minute timeout
    }
}

pub fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    let pin: u32 = rng.gen_range(0..1_000_000);
    format!("{:06}", pin)
}

pub fn create_pin_hash(pin: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(pin.as_bytes());
    hasher.finalize().to_vec()
}

pub fn verify_pin_hash(pin: &str, expected_hash: &[u8]) -> bool {
    let computed_hash = create_pin_hash(pin);
    computed_hash == expected_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_pin_format() {
        let pin = generate_pin();
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_pin_hash_verification() {
        let pin = "123456";
        let hash = create_pin_hash(pin);
        assert!(verify_pin_hash(pin, &hash));
        assert!(!verify_pin_hash("654321", &hash));
    }
}
