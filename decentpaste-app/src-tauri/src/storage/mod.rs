mod config;
mod peers;

pub use config::{load_settings, save_settings, AppSettings};
pub use peers::{get_data_dir, init_data_dir, DeviceIdentity, PairedPeer};
