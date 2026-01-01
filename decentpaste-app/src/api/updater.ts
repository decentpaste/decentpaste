import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { platform, arch } from '@tauri-apps/plugin-os';
import { getBundleType } from '@tauri-apps/api/app';
import { store } from '../state/store';

let currentUpdate: Update | null = null;

/**
 * Map OS plugin platform names to updater target names.
 * The OS plugin uses "macos" but the updater uses "darwin".
 */
function mapPlatformToTarget(platformName: string): string {
  switch (platformName) {
    case 'macos':
      return 'darwin';
    default:
      return platformName;
  }
}

/**
 * Map bundle type to target suffix.
 * Some bundle types like "AppImage" need to be lowercased.
 */
function mapBundleType(bundleType: string | null): string | null {
  if (!bundleType) return null;
  return bundleType.toLowerCase();
}

/**
 * Build the updater target string from Tauri platform APIs.
 *
 * This constructs targets like:
 * - "linux-x86_64-deb" for Debian packages
 * - "linux-x86_64-appimage" for AppImages
 * - "windows-x86_64-nsis" for NSIS installers
 * - "darwin-aarch64" for macOS (no bundle_type suffix needed)
 *
 * Using explicit targets ensures the updater fetches the correct artifact
 * from the release manifest, matching how the app was originally installed.
 */
async function getUpdaterTarget(): Promise<string> {
  const [os, architecture, bundleType] = await Promise.all([platform(), arch(), getBundleType()]);

  // Map platform name (e.g., "macos" -> "darwin")
  const targetOs = mapPlatformToTarget(os);

  // Build base target: os-arch (e.g., "linux-x86_64")
  let target = `${targetOs}-${architecture}`;

  // Append bundle type for platforms with multiple installer formats
  // This matches the keys in latest.json (e.g., "linux-x86_64-deb")
  const mappedBundle = mapBundleType(bundleType);
  if (mappedBundle) {
    target = `${target}-${mappedBundle}`;
  }

  console.debug(`Updater target: ${target} (os=${os}, arch=${architecture}, bundle=${bundleType})`);
  return target;
}

/**
 * Check for available updates.
 * Uses platform info to select the correct update artifact.
 */
export async function checkForUpdates(): Promise<void> {
  store.set('updateStatus', 'checking');
  store.set('updateError', null);

  try {
    // Get the appropriate target for this platform/installer combination
    const target = await getUpdaterTarget();

    // Pass explicit target to ensure correct artifact is selected
    const update = await check({ target });

    if (update) {
      currentUpdate = update;
      store.set('updateInfo', {
        version: update.version,
        date: update.date ?? null,
        body: update.body ?? null,
      });
      store.set('updateStatus', 'available');
    } else {
      store.set('updateStatus', 'up-to-date');
      store.set('updateInfo', null);
    }
  } catch (error) {
    console.error('Failed to check for updates:', error);
    store.set('updateStatus', 'error');
    store.set('updateError', String(error));
  }
}

/**
 * Download and install the available update.
 * Shows progress during download.
 */
export async function downloadAndInstallUpdate(): Promise<void> {
  if (!currentUpdate) {
    store.set('updateStatus', 'error');
    store.set('updateError', 'No update available to install');
    return;
  }

  store.set('updateStatus', 'downloading');
  store.set('updateProgress', { downloaded: 0, total: null });

  try {
    await currentUpdate.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          store.set('updateProgress', {
            downloaded: 0,
            total: event.data.contentLength ?? null,
          });
          break;
        case 'Progress':
          store.update('updateProgress', (prev) => ({
            downloaded: (prev?.downloaded ?? 0) + event.data.chunkLength,
            total: prev?.total ?? null,
          }));
          break;
        case 'Finished':
          store.set('updateStatus', 'ready');
          break;
      }
    });

    store.set('updateStatus', 'installing');
    // Relaunch the app to apply the update
    await relaunch();
  } catch (error) {
    console.error('Failed to download/install update:', error);
    store.set('updateStatus', 'error');
    store.set('updateError', String(error));
  }
}

/**
 * Format bytes to human-readable string.
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
}

/**
 * Get the download progress as a percentage (0-100).
 */
export function getDownloadPercentage(): number {
  const progress = store.get('updateProgress');
  if (!progress || !progress.total) return 0;
  return Math.round((progress.downloaded / progress.total) * 100);
}

/**
 * Reset the update state to idle.
 */
export function resetUpdateState(): void {
  currentUpdate = null;
  store.set('updateStatus', 'idle');
  store.set('updateInfo', null);
  store.set('updateProgress', null);
  store.set('updateError', null);
}
