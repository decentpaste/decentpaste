/**
 * Platform detection utilities for conditional feature support.
 * Uses navigator.userAgent which is reliable in Tauri WebViews.
 */

/**
 * Returns true if running on Android.
 */
export function isAndroid(): boolean {
  return /Android/i.test(navigator.userAgent);
}

/**
 * Returns true if running on iOS (iPhone, iPad, iPod).
 */
export function isIOS(): boolean {
  return /iPhone|iPad|iPod/i.test(navigator.userAgent);
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
