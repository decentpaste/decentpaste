//! Secure vault storage module using IOTA Stronghold.
//!
//! This module provides encrypted storage for sensitive data including:
//! - Paired peer shared secrets
//! - Clipboard history
//! - Device identity and keys
//!
//! The vault is protected by a user PIN which is transformed via Argon2id
//! into an encryption key.

pub mod auth;
pub mod error;
pub mod manager;
pub mod salt;

pub use auth::{AuthMethod, VaultStatus};
pub use error::{VaultError, VaultResult};
pub use manager::VaultManager;
pub use salt::{delete_salt, get_or_create_salt};
