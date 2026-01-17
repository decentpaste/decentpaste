import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  AuthMethod,
  ClipboardEntry,
  DeviceInfo,
  DiscoveredPeer,
  NetworkStatus,
  PairedPeer,
  PairingSession,
  SecretStorageStatus,
  VaultStatus,
} from './types';

// Network commands
export async function getNetworkStatus(): Promise<NetworkStatus> {
  return invoke('get_network_status');
}

export async function startNetwork(): Promise<void> {
  return invoke('start_network');
}

export async function stopNetwork(): Promise<void> {
  return invoke('stop_network');
}

/**
 * Force reconnection to all discovered peers.
 * Call this when the app resumes from background on mobile.
 */
export async function reconnectPeers(): Promise<void> {
  return invoke('reconnect_peers');
}

/** Summary of connection status after refresh */
export interface ConnectionSummary {
  total_peers: number;
  success: boolean;
}

/**
 * Refresh connections to all paired peers.
 * This is an awaitable operation that returns when all dial attempts complete or timeout.
 * Use this for the refresh button instead of reconnectPeers for better feedback.
 */
export async function refreshConnections(): Promise<ConnectionSummary> {
  return invoke('refresh_connections');
}

/**
 * Update app visibility state in the backend.
 * This ensures backend is the single source of truth for foreground state.
 * Call this when document.visibilityState changes.
 */
export async function setAppVisibility(visible: boolean): Promise<void> {
  return invoke('set_app_visibility', { visible });
}

/** Response from processPendingClipboard */
export interface PendingClipboardResponse {
  content: string;
  from_device: string;
}

/**
 * Process any pending clipboard content received while app was in background.
 * Call this when the app becomes visible on mobile.
 * Returns the pending clipboard if any was waiting, null otherwise.
 */
export async function processPendingClipboard(): Promise<PendingClipboardResponse | null> {
  return invoke('process_pending_clipboard');
}

// Peer management
export async function getDiscoveredPeers(): Promise<DiscoveredPeer[]> {
  return invoke('get_discovered_peers');
}

export async function getPairedPeers(): Promise<PairedPeer[]> {
  return invoke('get_paired_peers');
}

export async function removePairedPeer(peerId: string): Promise<void> {
  return invoke('remove_paired_peer', { peerId });
}

// Pairing flow
export async function initiatePairing(peerId: string): Promise<string> {
  return invoke('initiate_pairing', { peerId });
}

export async function respondToPairing(sessionId: string, accept: boolean): Promise<string | null> {
  return invoke('respond_to_pairing', { sessionId, accept });
}

export async function confirmPairing(sessionId: string, pin: string): Promise<boolean> {
  return invoke('confirm_pairing', { sessionId, pin });
}

export async function cancelPairing(sessionId: string): Promise<void> {
  return invoke('cancel_pairing', { sessionId });
}

export async function getPairingSessions(): Promise<PairingSession[]> {
  return invoke('get_pairing_sessions');
}

// Clipboard operations
export async function getClipboardHistory(limit?: number): Promise<ClipboardEntry[]> {
  return invoke('get_clipboard_history', { limit: limit ?? null });
}

export async function setClipboard(content: string): Promise<void> {
  return invoke('set_clipboard', { content });
}

export async function clearClipboardHistory(): Promise<void> {
  return invoke('clear_clipboard_history');
}

// Settings
export async function getSettings(): Promise<AppSettings> {
  return invoke('get_settings');
}

export async function updateSettings(settings: AppSettings): Promise<void> {
  return invoke('update_settings', { settings });
}

export async function getDeviceInfo(): Promise<DeviceInfo> {
  return invoke('get_device_info');
}

// Vault commands - Secure storage authentication and management

/**
 * Get the current vault status.
 * - NotSetup: First-time user, needs onboarding
 * - Locked: Vault exists but requires PIN to unlock
 * - Unlocked: Vault is open and data is accessible
 */
export async function getVaultStatus(): Promise<VaultStatus> {
  return invoke('get_vault_status');
}

/**
 * Check if secure storage (biometric/keyring) is available on this device.
 * Returns availability status and the specific method available.
 */
export async function checkSecretStorageAvailability(): Promise<SecretStorageStatus> {
  return invoke('check_secret_storage_availability');
}

/**
 * Get the authentication method used for the current vault.
 * Returns null if no vault is set up yet.
 */
export async function getVaultAuthMethod(): Promise<AuthMethod | null> {
  return invoke('get_vault_auth_method');
}

/**
 * Set up a new vault with PIN-based authentication.
 * The encryption key is derived from the PIN via Argon2id.
 * @param deviceName - The user's chosen device name
 * @param pin - The user's chosen PIN (4-8 digits)
 */
export async function setupVaultWithPin(deviceName: string, pin: string): Promise<void> {
  return invoke('setup_vault_with_pin', { deviceName, pin });
}

/**
 * Set up a new vault with secure storage (biometric/keyring).
 * Generates a random 256-bit key stored in platform secure storage.
 * @param deviceName - The user's chosen device name
 */
export async function setupVaultWithSecureStorage(deviceName: string): Promise<void> {
  return invoke('setup_vault_with_secure_storage', { deviceName });
}

/**
 * Set up a new vault with keychain + PIN (desktop only).
 * Provides 2-factor security by:
 * 1. Generating a random 256-bit vault key
 * 2. Encrypting it with a PIN-derived key (Argon2id)
 * 3. Storing the encrypted key in the OS keychain
 *
 * @param deviceName - The user's chosen device name
 * @param pin - The user's chosen PIN (4-8 digits)
 */
export async function setupVaultWithSecureStorageAndPin(deviceName: string, pin: string): Promise<void> {
  return invoke('setup_vault_with_secure_storage_and_pin', { deviceName, pin });
}

/**
 * Legacy wrapper for setupVaultWithPin (for backward compatibility).
 * @deprecated Use setupVaultWithPin or setupVaultWithSecureStorage instead.
 */
export async function setupVault(deviceName: string, pin: string, _authMethod: AuthMethod): Promise<void> {
  return setupVaultWithPin(deviceName, pin);
}

/**
 * Unlock an existing vault.
 * Auto-detects the auth method from stored config.
 * - SecureStorage: Triggers biometric prompt (mobile) or retrieves from keyring (desktop)
 * - PIN: Requires the pin parameter
 * - SecureStorageWithPin (desktop): Retrieves encrypted key from keychain, decrypts with PIN
 * @param pin - Optional PIN (required if vault uses PIN or SecureStorageWithPin auth)
 */
export async function unlockVault(pin?: string): Promise<void> {
  return invoke('unlock_vault', { pin: pin ?? null });
}

/**
 * Lock the vault, flushing all data and clearing keys from memory.
 * After locking, the user must enter their PIN to access data again.
 */
export async function lockVault(): Promise<void> {
  return invoke('lock_vault');
}

/**
 * Reset the vault, destroying all encrypted data.
 * This is a destructive operation - user must go through onboarding again.
 */
export async function resetVault(): Promise<void> {
  return invoke('reset_vault');
}

/**
 * Flush current app state to the vault (safety net).
 * With flush-on-write pattern, data is already persisted on mutation.
 * This is called before backgrounding on mobile as an extra safety measure.
 */
export async function flushVault(): Promise<void> {
  return invoke('flush_vault');
}

// ============================================================================
// Share Intent Handling - For Android "share with" functionality
// ============================================================================

/**
 * Result of handling shared content from Android share intent.
 * This is a DTO - the UI decides how to present these values to the user.
 */
export interface ShareResult {
  /** Total number of paired peers */
  totalPeers: number;
  /** Whether the content was added to clipboard history */
  addedToHistory: boolean;
  /** Whether the share operation succeeded */
  success: boolean;
}

/**
 * Format a ShareResult DTO into a user-friendly message.
 * Centralizes the message logic so UI can present it consistently.
 */
export function formatShareResultMessage(result: ShareResult): string {
  if (result.success) {
    return `Sent to ${result.totalPeers} device(s)`;
  } else {
    return `Saved to history (${result.totalPeers} device(s))`;
  }
}

/**
 * Handle shared content received from Android share intent.
 *
 * This is called by the frontend after detecting pending share content
 * via getPendingShare() polling from the decentshare plugin. It:
 * 1. Verifies the vault is unlocked
 * 2. Ensures peers are connected (awaitable, with timeout)
 * 3. Shares the content with all connected paired peers
 * 4. Adds the content to clipboard history
 *
 * @param content - The shared text content
 * @returns DTO with peer counts - UI decides how to present this
 * @throws VaultLocked if vault is not unlocked
 * @throws NoPeersAvailable if there are no paired peers
 */
export async function handleSharedContent(content: string): Promise<ShareResult> {
  return invoke('handle_shared_content', { content });
}
