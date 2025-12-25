use aes_gcm::aead::OsRng;
use chrono::Utc;
use uuid::Uuid;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::Result;
use crate::storage::DeviceIdentity;

/// Generate a new device identity with X25519 keypair for ECDH key exchange.
///
/// This creates the identity in memory only. The caller is responsible for
/// persisting it to the vault via `VaultManager::set_device_identity()`.
pub fn generate_device_identity(device_name: &str) -> DeviceIdentity {
    // Generate a unique device ID
    let device_id = Uuid::new_v4().to_string();

    // Generate X25519 keypair for ECDH key exchange during pairing
    let private_key = StaticSecret::random_from_rng(OsRng);
    let public_key = PublicKey::from(&private_key);

    DeviceIdentity {
        device_id,
        device_name: device_name.to_string(),
        public_key: public_key.as_bytes().to_vec(),
        private_key: Some(private_key.as_bytes().to_vec()),
        created_at: Utc::now(),
    }
}

// NOTE: get_or_create_identity() was removed as it used legacy plaintext storage.
// Device identity is now created during vault setup and stored in the encrypted vault.

/// Derive a shared secret using X25519 ECDH
/// Takes our private key and the peer's public key, returns a 32-byte shared secret
pub fn derive_shared_secret(our_private_key: &[u8], their_public_key: &[u8]) -> Result<Vec<u8>> {
    use crate::error::DecentPasteError;

    if our_private_key.len() != 32 {
        return Err(DecentPasteError::Encryption(
            "Private key must be 32 bytes".into(),
        ));
    }
    if their_public_key.len() != 32 {
        return Err(DecentPasteError::Encryption(
            "Public key must be 32 bytes".into(),
        ));
    }

    // Convert bytes to X25519 types
    let private_bytes: [u8; 32] = our_private_key
        .try_into()
        .map_err(|_| DecentPasteError::Encryption("Invalid private key".into()))?;
    let public_bytes: [u8; 32] = their_public_key
        .try_into()
        .map_err(|_| DecentPasteError::Encryption("Invalid public key".into()))?;

    let our_secret = StaticSecret::from(private_bytes);
    let their_public = PublicKey::from(public_bytes);

    // Perform ECDH to derive shared secret
    let shared_secret = our_secret.diffie_hellman(&their_public);

    Ok(shared_secret.as_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_identity_creates_valid_keys() {
        let identity = generate_device_identity("Test Device");

        assert_eq!(identity.public_key.len(), 32);
        assert!(identity.private_key.is_some());
        assert_eq!(identity.private_key.as_ref().unwrap().len(), 32);
        assert!(!identity.device_id.is_empty());
        assert_eq!(identity.device_name, "Test Device");
    }

    #[test]
    fn test_ecdh_key_exchange() {
        // Simulate two devices
        let alice = generate_device_identity("Alice");
        let bob = generate_device_identity("Bob");

        // Alice derives shared secret using her private key + Bob's public key
        let alice_shared =
            derive_shared_secret(alice.private_key.as_ref().unwrap(), &bob.public_key).unwrap();

        // Bob derives shared secret using his private key + Alice's public key
        let bob_shared =
            derive_shared_secret(bob.private_key.as_ref().unwrap(), &alice.public_key).unwrap();

        // Both should derive the same shared secret!
        assert_eq!(alice_shared, bob_shared);
        assert_eq!(alice_shared.len(), 32);
    }

    #[test]
    fn test_different_keypairs_produce_different_secrets() {
        let alice = generate_device_identity("Alice");
        let bob = generate_device_identity("Bob");
        let charlie = generate_device_identity("Charlie");

        let alice_bob =
            derive_shared_secret(alice.private_key.as_ref().unwrap(), &bob.public_key).unwrap();

        let alice_charlie =
            derive_shared_secret(alice.private_key.as_ref().unwrap(), &charlie.public_key).unwrap();

        // Different peer pairs should have different shared secrets
        assert_ne!(alice_bob, alice_charlie);
    }
}
