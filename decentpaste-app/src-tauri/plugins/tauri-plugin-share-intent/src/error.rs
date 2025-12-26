use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to access share intent: {0}")]
    AccessError(String),

    #[error("No pending share content")]
    NoPendingContent,

    #[error("Platform not supported")]
    PlatformNotSupported,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
