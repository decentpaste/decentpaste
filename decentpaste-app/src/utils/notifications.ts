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

    if (!permissionGranted) {
      const permission = await requestPermission();
      permissionGranted = permission === 'granted';
    }
  } catch {
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

  if (hasPermission) {
    try {
      sendNotification({ title, body });
    } catch {
      // Silently fail - notification is not critical
    }
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
