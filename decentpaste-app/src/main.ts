import './styles.css';
import { initApp } from './app';
import { reconnectPeers } from './api/commands';
import { store } from './state/store';

document.addEventListener('DOMContentLoaded', async () => {
  const root = document.getElementById('app');
  if (root) {
    await initApp(root);
  }

  // Handle app visibility changes (especially important for mobile)
  // When app returns from background, reconnect to peers
  document.addEventListener('visibilitychange', async () => {
    if (document.visibilityState === 'visible') {
      store.set('isWindowVisible', true);
      store.set('isMinimizedToTray', false); // Window is now visible, no longer in tray
      console.log('App became visible, reconnecting to peers...');
      try {
        await reconnectPeers();
      } catch (e) {
        console.error('Failed to reconnect peers:', e);
      }
    }
  });
});
