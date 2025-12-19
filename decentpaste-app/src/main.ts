import './styles.css';
import { initApp } from './app';
import { reconnectPeers, processPendingClipboard } from './api/commands';
import { store } from './state/store';
import { checkForUpdates } from './api/updater';

document.addEventListener('DOMContentLoaded', async () => {
  const root = document.getElementById('app');
  if (root) {
    await initApp(root);
  }

  // Handle app visibility changes (especially important for mobile)
  // When app returns from background, reconnect to peers and process pending clipboard
  document.addEventListener('visibilitychange', async () => {
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
