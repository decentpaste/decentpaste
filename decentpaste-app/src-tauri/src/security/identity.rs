use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use uuid::Uuid;

use crate::error::Result;
use crate::storage::{load_device_identity, load_settings, save_device_identity, DeviceIdentity};

pub fn generate_device_identity(device_name: &str) -> DeviceIdentity {
    // Generate a unique device ID
    let device_id = Uuid::new_v4().to_string();

    // Generate keypair for pairing (simplified - using random bytes)
    // In production, use proper asymmetric crypto like X25519
    let mut public_key = vec![0u8; 32];
    let mut private_key = vec![0u8; 32];
    OsRng.fill_bytes(&mut public_key);
    OsRng.fill_bytes(&mut private_key);

    DeviceIdentity {
        device_id,
        device_name: device_name.to_string(),
        public_key,
        private_key: Some(private_key),
        created_at: Utc::now(),
    }
}

pub fn get_or_create_identity() -> Result<DeviceIdentity> {
    // Try to load existing identity
    if let Some(identity) = load_device_identity()? {
        return Ok(identity);
    }

    // Create new identity
    let settings = load_settings()?;
    let identity = generate_device_identity(&settings.device_name);

    // Save it
    save_device_identity(&identity)?;

    Ok(identity)
}
