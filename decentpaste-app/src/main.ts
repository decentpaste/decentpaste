import './styles.css';
import { initApp } from './app';
import {
  reconnectPeers,
  processPendingClipboard,
  flushVault,
  handleSharedContent,
  setAppVisibility,
  formatShareResultMessage,
} from './api/commands';
import { store } from './state/store';
import { checkForUpdates } from './api/updater';
import { isDesktop, isMobile } from './utils/platform';
import { getPendingShare } from 'tauri-plugin-decentshare-api';
import { onOpenUrl } from '@tauri-apps/plugin-deep-link';

// Track if the app has fully initialized (Tauri IPC is ready)
let appInitialized = false;

/**
 * Handle shared content from Android share intent or iOS share extension.
 *
 * This is called when:
 * 1. The app finds pending shared content on startup or visibility change
 * 2. iOS: Deep link received from share extension (decentpaste://share)
 *
 * If vault is locked, the content is stored in pendingShare and processed after unlock.
 * If vault is unlocked, the content is shared immediately with all paired peers.
 */
async function handleShareIntent(content: string): Promise<void> {
  console.log(`Handling share intent (${content.length} chars)`);

  const vaultStatus = store.get('vaultStatus');

  if (vaultStatus !== 'Unlocked') {
    // Vault is locked - store content for processing after unlock
    console.log('Vault is locked, storing pending share');
    store.set('pendingShare', content);
    store.addToast('Unlock to share with your devices', 'info');
    return;
  }

  // Vault is unlocked - share immediately
  try {
    store.addToast('Sharing with your devices...', 'info');

    const result = await handleSharedContent(content);
    const message = formatShareResultMessage(result);

    // Determine toast type based on result
    const toastType = result.peersReached > 0 ? 'success' : 'info';
    store.addToast(message, toastType);
  } catch (error) {
    console.error('Failed to handle shared content:', error);

    // Handle specific error types
    const errorMessage = String(error);
    if (errorMessage.includes('VaultLocked')) {
      store.set('pendingShare', content);
      store.addToast('Unlock to share with your devices', 'info');
    } else if (errorMessage.includes('NoPeersAvailable')) {
      store.addToast('No paired devices. Pair a device first.', 'error');
    } else {
      store.addToast('Failed to share: ' + errorMessage, 'error');
    }
  }
}

/**
 * Check for and process any pending shared content from Android share intent.
 *
 * ## Why Retry with Backoff?
 * The Tauri IPC bridge can fail transiently when:
 * - App is resuming from background (WebView reinitializing)
 * - Android share intent arrives before Tauri bridge is fully ready
 * - Hot reload during development
 *
 * This retry handles **IPC reliability**, NOT network/peer connectivity.
 * Peer reconnection is handled separately by the backend's ensure_connected().
 *
 * ## Strategy
 * - 3 attempts with exponential backoff: 200ms, 400ms, 800ms delays
 * - On success: calls handleShareIntent() which invokes backend
 * - Backend's handleSharedContent() uses event-driven waiting for peers
 *
 * ## Flow
 * 1. getPendingShare() - Asks plugin for shared content
 * 2. handleShareIntent() - Sends to backend which:
 *    - Triggers peer reconnection (ensure_connected)
 *    - Waits for connections with proper async primitives (not polling)
 *    - Broadcasts via gossipsub
 *    - Returns honest status: "Sent to 2/3. 1 offline."
 */
async function checkForPendingShare(): Promise<void> {
  if (!isMobile()) {
    return;
  }

  const maxRetries = 3;
  const baseDelay = 200; // ms

  for (let attempt = 0; attempt < maxRetries; attempt++) {
    try {
      const pendingShare = await getPendingShare();
      if (pendingShare.hasPending && pendingShare.content) {
        console.log('Found pending share content');
        await handleShareIntent(pendingShare.content);
      }
      return; // Success - exit
    } catch (error) {
      const delay = baseDelay * Math.pow(2, attempt);
      console.warn(`Failed to check pending share (attempt ${attempt + 1}/${maxRetries}):`, error);

      if (attempt < maxRetries - 1) {
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }
  }

  console.error('All attempts to check pending share failed');
}

document.addEventListener('DOMContentLoaded', async () => {
  const root = document.getElementById('app');
  if (root) {
    await initApp(root);
  }

  // Mark app as initialized after initApp completes (Tauri IPC is now ready)
  appInitialized = true;

  // Check for pending share content from Android/iOS share intent
  // This is called after app init to handle content that arrived via share sheet
  await checkForPendingShare();

  // Listen for deep links from iOS share extension
  // When ShareExtension opens the app via decentpaste://share, this triggers
  // an immediate check for pending shared content (faster than visibility change)
  if (isMobile()) {
    try {
      await onOpenUrl((urls) => {
        if (urls.some((u) => u.startsWith('decentpaste://'))) {
          console.log('[Share] Deep link received from share extension');
          checkForPendingShare();
        }
      });
    } catch (e) {
      // Deep link plugin may not be available on all platforms
      console.debug('[Share] Deep link listener not available:', e);
    }
  }

  // Reset flag when page is unloading (prevents IPC errors during refresh)
  window.addEventListener('beforeunload', () => {
    appInitialized = false;
  });

  // Handle app visibility changes (especially important for mobile)
  // When app returns from background, reconnect to peers and process pending clipboard
  // When app goes to background, flush vault to persist data
  document.addEventListener('visibilitychange', async () => {
    // Skip if app hasn't fully initialized (prevents IPC errors during dev hot-reload)
    if (!appInitialized) return;

    const isVisible = document.visibilityState === 'visible';

    // Sync visibility state to backend FIRST (single source of truth)
    try {
      await setAppVisibility(isVisible);
    } catch (e) {
      console.error('Failed to set visibility:', e);
    }

    if (isVisible) {
      store.set('isWindowVisible', true);
      store.set('isMinimizedToTray', false); // Window is now visible, no longer in tray
      console.log('App became visible, reconnecting to peers...');
      try {
        await reconnectPeers();

        // Check for pending share content (Android share intent)
        // This handles the case where user shared to the app while it was backgrounded
        await checkForPendingShare();

        // Process any pending clipboard from background (Android)
        const pending = await processPendingClipboard();
        if (pending) {
          console.log(`Clipboard synced from ${pending.from_device}`);
          store.addToast(`Clipboard synced from ${pending.from_device}`, 'success');
        }
      } catch (e) {
        console.error('Failed to reconnect peers:', e);
      }
    } else {
      // App going to background - flush vault to persist data
      const vaultStatus = store.get('vaultStatus');
      if (vaultStatus === 'Unlocked') {
        console.log('App going to background, flushing vault...');
        try {
          await flushVault();
          console.log('Vault flushed successfully');
        } catch (e) {
          console.error('Failed to flush vault:', e);
        }
      }
    }
  });

  // Periodic update check every minute (desktop only)
  // Mobile platforms use app stores for updates (Google Play, App Store)
  if (isDesktop()) {
    const checkUpdatesQuietly = () => {
      checkForUpdates().catch((e) => {
        // Silently fail if offline - user can manually check in Settings
        console.debug('Update check failed (offline?):', e);
      });
    };

    // Initial check after 10 seconds (let app fully initialize)
    setTimeout(checkUpdatesQuietly, 10000);

    // Then check every minute
    setInterval(checkUpdatesQuietly, 60000);
  }
});

// Export for use from app.ts after vault unlock
export { handleShareIntent };
