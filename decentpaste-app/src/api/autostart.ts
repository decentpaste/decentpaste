import { enable, disable, isEnabled } from '@tauri-apps/plugin-autostart';
import { isDesktop } from '../utils/platform';

/**
 * Check if autostart is currently enabled.
 * Returns false on mobile platforms (plugin not available).
 */
export async function getAutostart(): Promise<boolean> {
  if (!isDesktop()) return false;
  return await isEnabled();
}

/**
 * Enable or disable autostart.
 * No-op on mobile platforms.
 */
export async function setAutostart(enabled: boolean): Promise<void> {
  if (!isDesktop()) return;
  if (enabled) {
    await enable();
  } else {
    await disable();
  }
}
