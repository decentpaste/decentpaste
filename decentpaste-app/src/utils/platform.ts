/**
 * Platform detection utilities for conditional feature support.
 *
 * Uses Tauri's @tauri-apps/plugin-os for accurate platform detection.
 * This is essential because iPad reports as Mac in its user agent (since iPadOS 13+),
 * which would cause desktop-only code paths to execute on iPad.
 *
 * The plugin uses Rust's compile-time cfg!(target_os) which is always accurate.
 */

import { platform } from '@tauri-apps/plugin-os';

// Cached platform value, initialized once at app startup
let cachedPlatform: string | null = null;

/**
 * Initialize platform detection. Must be called once at app startup before
 * any platform checks are made.
 *
 * This caches the platform value so subsequent calls to isAndroid(), isIOS(),
 * etc. are synchronous and fast.
 */
export async function initPlatform(): Promise<void> {
  cachedPlatform = platform();
}

/**
 * Returns true if running on Android.
 */
export function isAndroid(): boolean {
  return cachedPlatform === 'android';
}

/**
 * Returns true if running on iOS (iPhone, iPad, iPod).
 */
export function isIOS(): boolean {
  return cachedPlatform === 'ios';
}

/**
 * Returns true if running on a mobile platform (Android or iOS).
 * Use this to disable features that are desktop-only, like in-app updates.
 */
export function isMobile(): boolean {
  return isAndroid() || isIOS();
}

/**
 * Returns true if running on a desktop platform (not mobile).
 */
export function isDesktop(): boolean {
  return !isMobile();
}
