// Network types
export type NetworkStatus = 'Disconnected' | 'Connecting' | 'Connected' | { Error: string };

export interface DiscoveredPeer {
  peer_id: string;
  device_name: string | null;
  addresses: string[];
  discovered_at: string;
  is_paired: boolean;
}

export interface PairedPeer {
  peer_id: string;
  device_name: string;
  paired_at: string;
  last_seen: string | null;
}

export interface ConnectedPeer {
  peer_id: string;
  device_name: string;
  connected_at: string;
}

// Clipboard types
export interface ClipboardEntry {
  id: string;
  content: string;
  content_hash: string;
  timestamp: string;
  origin_device_id: string;
  origin_device_name: string;
  is_local: boolean;
}

// Pairing types
export type PairingState =
  | 'Initiated'
  | 'AwaitingPinConfirmation'
  | 'AwaitingPeerConfirmation'
  | 'Completed'
  | { Failed: string };

export interface PairingSession {
  session_id: string;
  peer_id: string;
  peer_name: string | null;
  pin: string | null;
  state: PairingState;
  is_initiator: boolean;
  created_at: string;
}

// Vault types
export type VaultStatus = 'NotSetup' | 'Locked' | 'Unlocked';
export type AuthMethod = 'pin' | 'secure_storage' | 'secure_storage_with_pin';

// Secure storage types (from tauri-plugin-decentsecret)
export type SecretStorageMethod =
  | 'androidBiometric'
  | 'iOSBiometric'
  | 'macOSKeychain'
  | 'windowsCredentialManager'
  | 'linuxSecretService';

export interface SecretStorageStatus {
  available: boolean;
  method: SecretStorageMethod | null;
  unavailableReason: string | null;
}

// Settings types
export interface AppSettings {
  device_name: string;
  auto_sync_enabled: boolean;
  clipboard_history_limit: number;
  /** Whether to persist clipboard history across app restarts */
  keep_history: boolean;
  show_notifications: boolean;
  clipboard_poll_interval_ms: number;
  /** Authentication method for vault access ('pin', 'secure_storage', or 'secure_storage_with_pin') */
  auth_method: AuthMethod | null;
  /** Whether to hide clipboard content in the UI (privacy mode) */
  hide_clipboard_content: boolean;
  /** Auto-lock timeout in minutes. 0 means never auto-lock */
  auto_lock_minutes: number;
}

// Device info
export interface DeviceInfo {
  device_id: string;
  peer_id: string | null;
}

// Event payloads
export interface PairingRequestPayload {
  sessionId: string;
  peerId: string;
  deviceName: string;
}

export interface PairingPinPayload {
  sessionId: string;
  pin: string;
  peerDeviceName: string;
}

export interface PairingCompletePayload {
  sessionId: string;
  peerId: string;
  deviceName: string;
}

export interface PairingFailedPayload {
  sessionId: string;
  error: string;
}

export interface ClipboardBroadcastPayload {
  id: string;
  peerCount: number;
}

export interface PeerNameUpdatedPayload {
  peerId: string;
  deviceName: string;
}

// Update types
export interface UpdateInfo {
  version: string;
  date: string | null;
  body: string | null;
}

export interface UpdateProgress {
  downloaded: number;
  total: number | null;
}

export type UpdateStatus =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'installing'
  | 'up-to-date'
  | 'error';
