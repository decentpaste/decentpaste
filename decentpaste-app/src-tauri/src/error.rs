use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecentPasteError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Clipboard error: {0}")]
    Clipboard(String),

    #[error("Pairing error: {0}")]
    Pairing(String),

    #[error("Encryption error: {0}")]
    Encryption(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Channel send error")]
    ChannelSend,

    #[error("Channel receive error")]
    ChannelReceive,

    #[error("Peer not found: {0}")]
    PeerNotFound(String),

    #[error("Already paired with peer: {0}")]
    AlreadyPaired(String),

    #[error("Invalid PIN")]
    InvalidPin,

    #[error("Pairing timeout")]
    PairingTimeout,

    #[error("Not initialized")]
    NotInitialized,
}

impl serde::Serialize for DecentPasteError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DecentPasteError>;
