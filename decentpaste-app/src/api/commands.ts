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
  connected: number;
  failed: number;
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

/** Manually share clipboard content with paired peers (useful on mobile) */
export async function shareClipboardContent(content: string): Promise<void> {
  return invoke('share_clipboard_content', { content });
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
 * Set up a new vault during first-time onboarding.
 * Creates an encrypted Stronghold vault protected by the user's PIN.
 * @param deviceName - The user's chosen device name
 * @param pin - The user's chosen PIN (4-8 digits)
 * @param authMethod - Auth method (currently only 'pin' is supported)
 */
export async function setupVault(deviceName: string, pin: string, authMethod: AuthMethod): Promise<void> {
  return invoke('setup_vault', {
    deviceName,
    pin,
    authMethod,
  });
}

/**
 * Unlock an existing vault with the user's PIN.
 * On success, loads all encrypted data and starts network/clipboard services.
 * @param pin - The user's PIN
 */
export async function unlockVault(pin: string): Promise<void> {
  return invoke('unlock_vault', { pin });
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
  /** Number of peers that were online and received the content */
  peersReached: number;
  /** Number of peers that were offline */
  peersOffline: number;
  /** Whether the content was added to clipboard history */
  addedToHistory: boolean;
}

/**
 * Format a ShareResult DTO into a user-friendly message.
 * Centralizes the message logic so UI can present it consistently.
 */
export function formatShareResultMessage(result: ShareResult): string {
  if (result.peersReached === result.totalPeers) {
    // All peers received the content
    return `Sent to ${result.totalPeers} device(s)`;
  } else if (result.peersReached > 0) {
    // Some peers received, some offline
    return `Sent to ${result.peersReached}/${result.totalPeers}. ${result.peersOffline} offline.`;
  } else {
    // No peers reachable - saved to history for later sync
    return `Saved to history. ${result.totalPeers} device(s) offline.`;
  }
}

/**
 * Handle shared content received from Android share intent.
 *
 * This is called by the frontend after receiving a "share-received" event
 * from the decentshare plugin. It:
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
