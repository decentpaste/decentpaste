import './styles.css';
import { initApp } from './app';
import { reconnectPeers, processPendingClipboard, flushVault, handleSharedContent } from './api/commands';
import { store } from './state/store';
import { checkForUpdates } from './api/updater';
import { isDesktop, isMobile } from './utils/platform';
import { getPendingShare } from 'tauri-plugin-decentshare-api';

// Track if the app has fully initialized (Tauri IPC is ready)
let appInitialized = false;

/**
 * Handle shared content from Android share intent.
 *
 * This is called when:
 * 1. The app finds pending shared content on startup or visibility change
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

    if (result.success) {
      store.addToast(result.message || `Shared with ${result.peerCount} device(s)`, 'success');
    } else {
      store.addToast('Failed to share content', 'error');
    }
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
 * This uses a command-based approach to avoid race conditions with events.
 */
async function checkForPendingShare(): Promise<void> {
  if (!isMobile()) {
    return;
  }

  try {
    const pendingShare = await getPendingShare();
    if (pendingShare.hasPending && pendingShare.content) {
      console.log('Found pending share content');
      await handleShareIntent(pendingShare.content);
    }
  } catch (error) {
    console.error('Failed to check for pending share:', error);
  }
}

document.addEventListener('DOMContentLoaded', async () => {
  const root = document.getElementById('app');
  if (root) {
    await initApp(root);
  }

  // Mark app as initialized after initApp completes (Tauri IPC is now ready)
  appInitialized = true;

  // Check for pending share content from Android share intent
  // This is called after app init to handle content that arrived via share sheet
  await checkForPendingShare();

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

    if (document.visibilityState === 'visible') {
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
    } else if (document.visibilityState === 'hidden') {
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

// Export handleShareIntent for use from app.ts after vault unlock
export { handleShareIntent };
