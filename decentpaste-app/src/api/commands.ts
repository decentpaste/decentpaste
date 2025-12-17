import { invoke } from '@tauri-apps/api/core';
import type {
  AppSettings,
  ClipboardEntry,
  DeviceInfo,
  DiscoveredPeer,
  NetworkStatus,
  PairedPeer,
  PairingSession,
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
