import './styles.css';
import { initApp } from './app';
import { reconnectPeers, processPendingClipboard, flushVault, shareClipboardContent } from './api/commands';
import { store } from './state/store';
import { checkForUpdates } from './api/updater';
import { isDesktop, isMobile } from './utils/platform';
import { listen } from '@tauri-apps/api/event';

// Store pending share content for after vault unlock (mobile only)
let pendingShareContent: string | null = null;

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

  // Listen for share intents from mobile platforms (Android/iOS)
  // When user shares content from another app and selects DecentPaste
  if (isMobile()) {
    // Listen via Tauri event system
    listen<{ content: string; source: string }>('share-intent-received', async (event) => {
      console.log('Share intent received (Tauri event):', event.payload);
      await handleShareIntent(event.payload.content);
    });

    // Also listen via DOM CustomEvent as fallback
    window.addEventListener('share-intent-received', async (event: Event) => {
      const customEvent = event as CustomEvent<{ content: string; source: string }>;
      console.log('Share intent received (DOM event):', customEvent.detail);
      if (customEvent.detail?.content) {
        await handleShareIntent(customEvent.detail.content);
      }
    });
  }
});

/**
 * Handle shared content from another app via the OS share menu.
 * If vault is locked, stores for later. If unlocked, shares immediately.
 */
async function handleShareIntent(content: string): Promise<void> {
  if (!content || content.trim() === '') {
    console.log('Empty share intent, ignoring');
    return;
  }

  const vaultStatus = store.get('vaultStatus');
  console.log(`Processing share intent, vault status: ${vaultStatus}`);

  if (vaultStatus !== 'Unlocked') {
    // Store for later processing after unlock
    pendingShareContent = content;
    store.addToast('Unlock to share content with your devices', 'info');
    console.log('Stored share intent for after unlock');
    return;
  }

  // Vault is unlocked, process immediately
  await processShareIntentContent(content);
}

/**
 * Actually share the content with paired devices.
 * Reconnects to peers first (important for mobile resume) then broadcasts.
 */
async function processShareIntentContent(content: string): Promise<void> {
  try {
    // Reconnect to peers first (important for mobile resume)
    console.log('Reconnecting to peers...');
    await reconnectPeers();

    // Small delay for reconnection to establish
    await new Promise((resolve) => setTimeout(resolve, 500));

    // Share the content
    console.log('Sharing content with peers...');
    await shareClipboardContent(content);

    store.addToast('Content shared with your devices!', 'success');
    console.log('Share intent processed successfully');
  } catch (error) {
    console.error('Failed to process share intent:', error);
    const message = error instanceof Error ? error.message : String(error);

    if (message.includes('No paired peers')) {
      store.addToast('No paired devices to share with', 'error');
    } else {
      store.addToast(`Failed to share: ${message}`, 'error');
    }
  }
}

/**
 * Check for pending share content and process it.
 * Call this after vault unlock completes.
 */
export async function checkPendingShareContent(): Promise<void> {
  if (pendingShareContent) {
    const content = pendingShareContent;
    pendingShareContent = null;
    console.log('Processing pending share content after unlock');
    await processShareIntentContent(content);
  }
}
