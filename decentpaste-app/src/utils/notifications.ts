/**
 * Native OS notification utilities
 *
 * Uses tauri-plugin-notification for system-level notifications
 * that work even when the app window is hidden (minimized to tray)
 */

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

let permissionGranted: boolean | null = null;

/**
 * Ensure notification permission is granted
 * Caches the result to avoid repeated permission checks
 */
export async function ensureNotificationPermission(): Promise<boolean> {
  if (permissionGranted !== null) {
    return permissionGranted;
  }

  try {
    permissionGranted = await isPermissionGranted();
    console.log('[Notification] Initial permission check:', permissionGranted);

    if (!permissionGranted) {
      console.log('[Notification] Requesting permission...');
      const permission = await requestPermission();
      console.log('[Notification] Permission response:', permission);
      permissionGranted = permission === 'granted';
    }
  } catch (e) {
    console.error('[Notification] Failed to check permission:', e);
    permissionGranted = false;
  }

  return permissionGranted;
}

/**
 * Send a native OS notification
 * Falls back silently if permissions not granted
 */
export async function showNotification(title: string, body: string): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  console.log('[Notification] Permission granted:', hasPermission);

  if (hasPermission) {
    try {
      console.log('[Notification] Sending:', { title, body });
      sendNotification({ title, body });
    } catch (e) {
      console.error('[Notification] Failed to send:', e);
    }
  } else {
    console.warn('[Notification] Permission not granted, skipping notification');
  }
}

/**
 * Show notification for clipboard received from another device
 */
export async function notifyClipboardReceived(deviceName: string): Promise<void> {
  await showNotification('Clipboard Received', `From ${deviceName}`);
}

/**
 * Show notification when app minimizes to tray (first time only)
 */
export async function notifyMinimizedToTray(): Promise<void> {
  await showNotification(
    'DecentPaste',
    'App is still running in the system tray. Click the tray icon to show.'
  );
}
