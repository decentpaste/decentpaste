use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use sha2::{Digest, Sha256};

use crate::error::{DecentPasteError, Result};

const NONCE_SIZE: usize = 12;

pub fn hash_content(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn encrypt_content(content: &[u8], shared_secret: &[u8]) -> Result<Vec<u8>> {
    if shared_secret.len() != 32 {
        return Err(DecentPasteError::Encryption(
            "Shared secret must be 32 bytes".into(),
        ));
    }

    let cipher = Aes256Gcm::new_from_slice(shared_secret)
        .map_err(|e| DecentPasteError::Encryption(e.to_string()))?;

    // Generate random nonce
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    // Encrypt
    let ciphertext = cipher
        .encrypt(nonce, content)
        .map_err(|e| DecentPasteError::Encryption(e.to_string()))?;

    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

pub fn decrypt_content(encrypted: &[u8], shared_secret: &[u8]) -> Result<Vec<u8>> {
    if shared_secret.len() != 32 {
        return Err(DecentPasteError::Encryption(
            "Shared secret must be 32 bytes".into(),
        ));
    }

    if encrypted.len() < NONCE_SIZE {
        return Err(DecentPasteError::Encryption("Data too short".into()));
    }

    let cipher = Aes256Gcm::new_from_slice(shared_secret)
        .map_err(|e| DecentPasteError::Encryption(e.to_string()))?;

    // Extract nonce and ciphertext
    let (nonce_bytes, ciphertext) = encrypted.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    // Decrypt
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| DecentPasteError::Encryption(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_content() {
        let hash1 = hash_content("test");
        let hash2 = hash_content("test");
        let hash3 = hash_content("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
