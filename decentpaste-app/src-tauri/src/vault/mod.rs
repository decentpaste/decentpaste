//! Secure vault storage module using AES-256-GCM encryption.
//!
//! This module provides encrypted storage for sensitive data including:
//! - Paired peer shared secrets
//! - Clipboard history
//! - Device identity and keys
//!
//! The vault key is obtained via one of two methods:
//! - **SecureStorage**: Random 256-bit key stored in platform secure storage
//!   (biometric on mobile, keyring on desktop)
//! - **PIN**: Key derived from user's PIN via Argon2id

pub mod auth;
pub mod auth_persistence;
pub mod error;
pub mod manager;
pub mod salt;
pub mod storage;

pub use auth::{AuthMethod, VaultStatus};
pub use auth_persistence::{delete_auth_method, load_auth_method, save_auth_method};
pub use error::{VaultError, VaultResult};
pub use manager::VaultManager;
pub use salt::{delete_salt, get_or_create_salt};
pub use storage::{VaultData, VaultKey};
