//! Secure vault storage module using IOTA Stronghold.
//!
//! This module provides encrypted storage for sensitive data including:
//! - Paired peer shared secrets
//! - Clipboard history
//! - Device identity and keys
//!
//! The vault is protected by a user PIN which is transformed via Argon2id
//! into an encryption key. On mobile devices, biometric authentication
//! can be used as an alternative unlock method.

pub mod auth;

pub use auth::{AuthMethod, VaultStatus};
