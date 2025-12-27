import { invoke } from '@tauri-apps/api/core';

/**
 * Response from the getPendingShare command.
 */
export interface PendingShareResponse {
  /** The shared text content, if any */
  content: string | null;
  /** Whether there was pending content */
  hasPending: boolean;
}

/**
 * Check if there's pending shared content from an Android share intent.
 *
 * This should be called after app initialization to handle content
 * that was shared before the webview was ready.
 *
 * @returns The pending shared content, if any
 */
export async function getPendingShare(): Promise<PendingShareResponse> {
  return await invoke<PendingShareResponse>('plugin:decentshare|get_pending_share');
}

/**
 * Clear the pending shared content after it's been processed.
 *
 * This should be called after successfully handling the shared content
 * to prevent it from being processed again.
 */
export async function clearPendingShare(): Promise<void> {
  await invoke('plugin:decentshare|clear_pending_share');
}
