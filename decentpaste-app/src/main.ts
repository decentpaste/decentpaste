import './styles.css';
import { initApp } from './app';
import { reconnectPeers, processPendingClipboard, flushVault } from './api/commands';
import { store } from './state/store';
import { checkForUpdates } from './api/updater';

// Track if the app has fully initialized (Tauri IPC is ready)
let appInitialized = false;

document.addEventListener('DOMContentLoaded', async () => {
  const root = document.getElementById('app');
  if (root) {
    await initApp(root);
  }

  // Mark app as initialized after initApp completes (Tauri IPC is now ready)
  appInitialized = true;

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

  // Periodic update check every minute (all platforms)
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
});
