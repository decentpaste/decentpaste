import { store, type Toast, type View } from './state/store';
import { eventManager } from './api/events';
import * as commands from './api/commands';
import { readText } from '@tauri-apps/plugin-clipboard-manager';
import { icon, type IconName } from './components/icons';
import { $, escapeHtml, formatTime, truncate } from './utils/dom';
import { getErrorMessage } from './utils/error';
import { notifyClipboardReceived, notifyMinimizedToTray } from './utils/notifications';
import { checkForUpdates, downloadAndInstallUpdate, formatBytes, getDownloadPercentage } from './api/updater';
import type { ClipboardEntry, DiscoveredPeer, PairedPeer } from './api/types';
// ?url suffix prevents race condition where Tauri webview loads before Vite is ready,
// causing "image/svg+xml is not a valid JavaScript MIME type" error on first load
import logoDark from './assets/logo_dark.svg?url';

class App {
  private root: HTMLElement;
  private pairingInProgress: boolean = false; // Guard against duplicate pairing operations
  private modalRenderPending: boolean = false; // Debounce modal renders
  private autoLockTimer: ReturnType<typeof setTimeout> | null = null; // Auto-lock timer

  constructor(rootElement: HTMLElement) {
    this.root = rootElement;
  }

  async init(): Promise<void> {
    // Setup event listeners from backend
    await eventManager.setup();
    this.setupEventHandlers();

    // Check vault status first - determines what data to load
    try {
      const vaultStatus = await commands.getVaultStatus();
      store.set('vaultStatus', vaultStatus);

      // Only load full app data if vault is unlocked
      if (vaultStatus === 'Unlocked') {
        await this.loadInitialData();
      } else {
        // For NotSetup/Locked states, just mark loading as complete
        // Data will be loaded after unlock/setup via loadDataAfterUnlock()
        store.set('isLoading', false);
      }
    } catch (error) {
      console.error('Failed to check vault status:', error);
      // Fallback: assume not setup if we can't check
      store.set('vaultStatus', 'NotSetup');
      store.set('isLoading', false);
    }

    // Render UI
    this.render();

    // Setup delegated event listeners (once, on root)
    this.setupDelegatedListeners();

    // Subscribe to state changes
    this.setupStateSubscriptions();

    // Setup auto-lock activity tracking
    this.setupActivityTracking();
  }

  /**
   * Setup event delegation on root element to prevent listener accumulation.
   * This is called once during init, not on every re-render.
   */
  private setupDelegatedListeners(): void {
    // Handle all click events via delegation
    this.root.addEventListener('click', async (e) => {
      const target = e.target as HTMLElement;

      // Navigation
      const navEl = target.closest('[data-nav]');
      if (navEl) {
        const view = navEl.getAttribute('data-nav') as View;
        store.set('currentView', view);
        return;
      }

      // Copy buttons with visual feedback animation
      const copyEl = target.closest('[data-copy]');
      if (copyEl) {
        const id = copyEl.getAttribute('data-copy');
        const history = store.get('clipboardHistory');
        const item = history.find((h) => h.id === id);
        if (item) {
          await commands.setClipboard(item.content);

          // Visual feedback: change to checkmark with green tint
          const button = copyEl as HTMLElement;
          button.innerHTML = icon('check', 16);
          button.classList.add('copied');

          // Revert after 800ms
          setTimeout(() => {
            button.innerHTML = icon('copy', 16);
            button.classList.remove('copied');
          }, 800);
        }
        return;
      }

      // Pair buttons
      const pairEl = target.closest('[data-pair]');
      if (pairEl) {
        const peerId = pairEl.getAttribute('data-pair');
        if (peerId) {
          const peers = store.get('discoveredPeers');
          const peer = peers.find((p) => p.peer_id === peerId);

          store.set('showPairingModal', true);
          store.set('pairingModalMode', 'initiate');
          store.set('activePairingSession', {
            session_id: '',
            peer_id: peerId,
            peer_name: peer?.device_name || null,
            pin: null,
            state: 'Initiated',
            is_initiator: true,
            created_at: new Date().toISOString(),
          });

          try {
            const sessionId = await commands.initiatePairing(peerId);
            store.update('activePairingSession', (s) => (s ? { ...s, session_id: sessionId } : null));
          } catch (error) {
            store.addToast(`Failed to initiate pairing: ${getErrorMessage(error)}`, 'error');
            store.set('showPairingModal', false);
            store.set('activePairingSession', null);
          }
        }
        return;
      }

      // Unpair buttons
      const unpairEl = target.closest('[data-unpair]');
      if (unpairEl) {
        const peerId = unpairEl.getAttribute('data-unpair');
        if (peerId) {
          try {
            await commands.removePairedPeer(peerId);
            store.removePairedPeer(peerId);
            store.addToast('Device unpaired', 'success');
          } catch (error) {
            store.addToast(`Failed to unpair: ${getErrorMessage(error)}`, 'error');
          }
        }
        return;
      }

      // Dismiss toasts
      const dismissEl = target.closest('[data-dismiss-toast]');
      if (dismissEl) {
        const id = dismissEl.getAttribute('data-dismiss-toast');
        if (id) store.removeToast(id);
        return;
      }

      // Refresh peers button - triggers reconnection to discovered peers
      if (target.closest('#btn-refresh-peers')) {
        try {
          await commands.reconnectPeers();
          store.addToast('Reconnecting to peers...', 'info');
        } catch (error) {
          store.addToast(`Failed to reconnect: ${getErrorMessage(error)}`, 'error');
        }
        return;
      }

      // Clear history buttons
      if (target.closest('#btn-clear-history') || target.closest('#btn-clear-all-history')) {
        try {
          await commands.clearClipboardHistory();
          store.set('clipboardHistory', []);
          store.addToast('History cleared', 'success');
        } catch (error) {
          store.addToast(`Failed to clear history: ${getErrorMessage(error)}`, 'error');
        }
        return;
      }

      // Share clipboard button
      if (target.closest('#btn-share-clipboard')) {
        try {
          const content = await readText();
          if (!content || content.trim() === '') {
            store.addToast('Clipboard is empty', 'error');
            return;
          }
          await commands.shareClipboardContent(content);
          store.addToast('Clipboard shared with peers', 'success');
        } catch (error) {
          store.addToast(`Failed to share: ${getErrorMessage(error)}`, 'error');
        }
        return;
      }

      // Toggle clipboard content visibility
      if (target.closest('#btn-toggle-visibility')) {
        const currentSetting = store.get('settings').hide_clipboard_content;
        const settings = { ...store.get('settings'), hide_clipboard_content: !currentSetting };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
        }
        return;
      }

      // Update buttons
      if (target.closest('#btn-check-update')) {
        checkForUpdates();
        return;
      }

      if (target.closest('#btn-download-update')) {
        downloadAndInstallUpdate();
        return;
      }

      // Lock vault button (header)
      const lockVaultBtn = target.closest('#btn-lock-vault') as HTMLButtonElement | null;
      if (lockVaultBtn) {
        lockVaultBtn.disabled = true;
        lockVaultBtn.innerHTML = icon('loader', 16, 'animate-spin');
        try {
          await commands.lockVault();
          // vaultStatus will be updated via event, triggering re-render to lock screen
        } catch (error) {
          store.addToast(`Failed to lock vault: ${getErrorMessage(error)}`, 'error');
          lockVaultBtn.disabled = false;
          lockVaultBtn.innerHTML = icon('lock', 16);
        }
        return;
      }

      // Pairing modal buttons
      const acceptBtn = target.closest('#btn-accept-pairing') as HTMLButtonElement | null;
      if (acceptBtn) {
        const session = store.get('activePairingSession');
        if (session && !acceptBtn.disabled && !this.pairingInProgress) {
          acceptBtn.disabled = true;
          this.pairingInProgress = true;
          acceptBtn.textContent = 'Accepting...';
          try {
            const pin = await commands.respondToPairing(session.session_id, true);
            if (pin) {
              store.update('activePairingSession', (s) => (s ? { ...s, pin, state: 'AwaitingPinConfirmation' } : null));
              store.set('pairingModalMode', 'confirm');
            }
          } catch (error) {
            store.addToast(`Failed to accept pairing: ${getErrorMessage(error)}`, 'error');
            acceptBtn.disabled = false;
            acceptBtn.textContent = 'Accept';
          } finally {
            this.pairingInProgress = false;
          }
        }
        return;
      }

      const rejectBtn = target.closest('#btn-reject-pairing') as HTMLButtonElement | null;
      if (rejectBtn) {
        const session = store.get('activePairingSession');
        if (session && !rejectBtn.disabled) {
          rejectBtn.disabled = true;
          try {
            await commands.respondToPairing(session.session_id, false);
          } catch (error) {
            store.addToast(`Failed to reject pairing: ${getErrorMessage(error)}`, 'error');
          } finally {
            store.set('showPairingModal', false);
            store.set('activePairingSession', null);
          }
        }
        return;
      }

      const confirmBtn = target.closest('#btn-confirm-pin') as HTMLButtonElement | null;
      if (confirmBtn) {
        const session = store.get('activePairingSession');
        if (session && session.pin && !confirmBtn.disabled) {
          confirmBtn.disabled = true;
          confirmBtn.textContent = 'Confirming...';
          try {
            const success = await commands.confirmPairing(session.session_id, session.pin);
            if (!success) {
              store.addToast('PIN verification failed', 'error');
              confirmBtn.disabled = false;
              confirmBtn.textContent = 'Confirm';
            }
          } catch (error) {
            store.addToast(`Failed to confirm pairing: ${getErrorMessage(error)}`, 'error');
            confirmBtn.disabled = false;
            confirmBtn.textContent = 'Confirm';
          }
        }
        return;
      }

      const cancelBtn = target.closest('#btn-cancel-pairing') as HTMLButtonElement | null;
      if (cancelBtn) {
        const session = store.get('activePairingSession');
        if (session && !cancelBtn.disabled) {
          cancelBtn.disabled = true;
          await commands.cancelPairing(session.session_id);
        }
        store.set('showPairingModal', false);
        store.set('activePairingSession', null);
        return;
      }

      // Lock screen: Unlock button
      const unlockBtn = target.closest('#btn-unlock') as HTMLButtonElement | null;
      if (unlockBtn) {
        const pinInput = document.getElementById('lock-pin-input') as HTMLInputElement | null;
        const errorEl = document.getElementById('lock-error');
        const pin = pinInput?.value || '';

        if (pin.length < 4) {
          if (errorEl) {
            errorEl.textContent = 'PIN must be at least 4 digits';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        unlockBtn.disabled = true;
        unlockBtn.innerHTML = `${icon('loader', 18, 'animate-spin')}<span>Unlocking...</span>`;

        try {
          await commands.unlockVault(pin);
          // Status will be updated via vault-status event
        } catch (error) {
          if (errorEl) {
            errorEl.textContent = getErrorMessage(error);
            errorEl.classList.remove('hidden');
          }
          unlockBtn.disabled = false;
          unlockBtn.innerHTML = `${icon('unlock', 18)}<span>Unlock</span>`;
          if (pinInput) {
            pinInput.value = '';
            pinInput.focus();
          }
        }
        return;
      }

      // Lock screen: Forgot PIN button - show reset confirmation
      if (target.closest('#btn-forgot-pin')) {
        store.set('showResetConfirmation', true);
        return;
      }

      // Reset confirmation: Cancel button
      if (target.closest('#btn-reset-cancel')) {
        store.set('showResetConfirmation', false);
        return;
      }

      // Reset confirmation: Confirm button
      const resetConfirmBtn = target.closest('#btn-reset-confirm') as HTMLButtonElement | null;
      if (resetConfirmBtn) {
        const resetInput = document.getElementById('reset-confirmation-input') as HTMLInputElement | null;
        const errorEl = document.getElementById('reset-error');
        const inputValue = resetInput?.value.trim() || '';

        if (inputValue !== 'RESET') {
          if (errorEl) {
            errorEl.textContent = 'Please type RESET to confirm';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        // Disable button and show loading
        resetConfirmBtn.disabled = true;
        resetConfirmBtn.innerHTML = `${icon('loader', 18, 'animate-spin')}<span>Resetting...</span>`;

        try {
          await commands.resetVault();
          store.set('showResetConfirmation', false);
          store.set('vaultStatus', 'NotSetup');
          store.set('onboardingStep', 'device-name');
          store.addToast('Vault reset. Please set up again.', 'info');
        } catch (error) {
          if (errorEl) {
            errorEl.textContent = getErrorMessage(error);
            errorEl.classList.remove('hidden');
          }
          resetConfirmBtn.disabled = false;
          resetConfirmBtn.innerHTML = `${icon('trash', 18)}<span>Reset Everything</span>`;
        }
        return;
      }

      // Onboarding: Step 1 - Continue from device name
      const step1ContinueBtn = target.closest('#btn-onboarding-step1-continue') as HTMLButtonElement | null;
      if (step1ContinueBtn) {
        const deviceNameInput = document.getElementById('onboarding-device-name') as HTMLInputElement | null;
        const errorEl = document.getElementById('onboarding-step1-error');
        const deviceName = deviceNameInput?.value.trim() || '';

        if (deviceName.length < 1) {
          if (errorEl) {
            errorEl.textContent = 'Please enter a device name';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        store.set('onboardingDeviceName', deviceName);
        store.set('onboardingStep', 'pin-setup');
        this.render();
        return;
      }

      // Onboarding: Step 2 - Back button
      if (target.closest('#btn-onboarding-step2-back')) {
        store.set('onboardingStep', 'device-name');
        this.render();
        return;
      }

      // Onboarding: Step 2 - Complete setup
      const completeSetupBtn = target.closest('#btn-onboarding-complete') as HTMLButtonElement | null;
      if (completeSetupBtn) {
        const pinInput = document.getElementById('onboarding-pin') as HTMLInputElement | null;
        const confirmInput = document.getElementById('onboarding-pin-confirm') as HTMLInputElement | null;
        const errorEl = document.getElementById('onboarding-step3-error');

        const pin = pinInput?.value || '';
        const confirmPin = confirmInput?.value || '';

        // Validate PIN length
        if (pin.length < 4 || pin.length > 8) {
          if (errorEl) {
            errorEl.textContent = 'PIN must be 4-8 digits';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        // Validate PIN is numeric
        if (!/^\d+$/.test(pin)) {
          if (errorEl) {
            errorEl.textContent = 'PIN must contain only digits';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        // Validate PIN confirmation matches
        if (pin !== confirmPin) {
          if (errorEl) {
            errorEl.textContent = 'PINs do not match';
            errorEl.classList.remove('hidden');
          }
          return;
        }

        // Disable button and show loading
        completeSetupBtn.disabled = true;
        completeSetupBtn.innerHTML = `${icon('loader', 18, 'animate-spin')}<span>Setting up...</span>`;

        try {
          const deviceName = store.get('onboardingDeviceName');

          await commands.setupVault(deviceName, pin, 'pin');

          // Reset onboarding state
          store.set('onboardingStep', null);
          store.set('onboardingDeviceName', '');

          // Vault status will be updated via event, triggering re-render
          store.addToast('Vault created successfully!', 'success');
        } catch (error) {
          if (errorEl) {
            errorEl.textContent = getErrorMessage(error);
            errorEl.classList.remove('hidden');
          }
          completeSetupBtn.disabled = false;
          completeSetupBtn.innerHTML = `${icon('check', 18)}<span>Complete Setup</span>`;
        }
        return;
      }
    });

    // Handle keydown events for PIN input (Enter to submit)
    this.root.addEventListener('keydown', (e) => {
      const target = e.target as HTMLElement;
      if (target.id === 'lock-pin-input' && e.key === 'Enter') {
        e.preventDefault();
        const unlockBtn = document.getElementById('btn-unlock') as HTMLButtonElement | null;
        unlockBtn?.click();
      }
    });

    // Handle change events for settings (needs separate listener due to event type)
    this.root.addEventListener('change', async (e) => {
      const target = e.target as HTMLInputElement | HTMLSelectElement;

      // Auto-sync toggle (Settings page and Dashboard)
      if (target.id === 'auto-sync-toggle' || target.id === 'dashboard-sync-toggle') {
        const checked = (target as HTMLInputElement).checked;
        const settings = { ...store.get('settings'), auto_sync_enabled: checked };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
          (target as HTMLInputElement).checked = !checked; // Revert
        }
        return;
      }

      // Auto-lock timer select
      if (target.id === 'auto-lock-select') {
        const value = parseInt((target as HTMLSelectElement).value, 10);
        const oldSettings = store.get('settings');
        const settings = { ...oldSettings, auto_lock_minutes: value };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
          (target as HTMLSelectElement).value = String(oldSettings.auto_lock_minutes);
        }
        return;
      }

      // Notifications toggle
      if (target.id === 'notifications-toggle') {
        const checked = (target as HTMLInputElement).checked;
        const settings = { ...store.get('settings'), show_notifications: checked };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
          (target as HTMLInputElement).checked = !checked;
        }
        return;
      }

      // History limit select
      if (target.id === 'history-limit-select') {
        const value = parseInt((target as HTMLSelectElement).value, 10);
        const oldSettings = store.get('settings');
        const settings = { ...oldSettings, clipboard_history_limit: value };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
          (target as HTMLSelectElement).value = String(oldSettings.clipboard_history_limit);
        }
        return;
      }

      // Keep history toggle (direct: checked = keep_history)
      if (target.id === 'keep-history-toggle') {
        const checked = (target as HTMLInputElement).checked;
        const settings = { ...store.get('settings'), keep_history: checked };
        try {
          await commands.updateSettings(settings);
          store.set('settings', settings);
        } catch (error) {
          store.addToast(`Failed to update settings: ${getErrorMessage(error)}`, 'error');
          (target as HTMLInputElement).checked = !checked;
        }
        return;
      }
    });

    // Handle blur events for device name input
    this.root.addEventListener(
      'blur',
      async (e) => {
        const target = e.target as HTMLInputElement;
        if (target.id === 'device-name-input') {
          const value = target.value.trim();
          if (value) {
            const settings = { ...store.get('settings'), device_name: value };
            try {
              await commands.updateSettings(settings);
              store.set('settings', settings);
            } catch (error) {
              store.addToast(`Failed to update device name: ${getErrorMessage(error)}`, 'error');
            }
          }
        }
      },
      true,
    ); // Use capture to ensure we get the event
  }

  private setupEventHandlers(): void {
    eventManager.on('networkStatus', (status) => {
      store.set('networkStatus', status);
    });

    eventManager.on('peerDiscovered', (peer) => {
      store.addDiscoveredPeer(peer);
    });

    eventManager.on('peerLost', (peerId) => {
      store.removeDiscoveredPeer(peerId);
    });

    eventManager.on('peerNameUpdated', (payload) => {
      store.updatePeerName(payload.peerId, payload.deviceName);
    });

    eventManager.on('clipboardReceived', (entry) => {
      store.addClipboardEntry(entry);

      // Use native notification ONLY when minimized to system tray
      // Otherwise use in-app toast (or skip if window just not focused)
      if (store.get('isMinimizedToTray')) {
        notifyClipboardReceived(entry.origin_device_name);
      } else {
        store.addToast(`Clipboard received from ${entry.origin_device_name}`, 'success');
      }
    });

    eventManager.on('clipboardSent', (entry) => {
      store.addClipboardEntry(entry);
    });

    eventManager.on('pairingRequest', (payload) => {
      store.set('showPairingModal', true);
      store.set('pairingModalMode', 'respond');
      store.set('activePairingSession', {
        session_id: payload.sessionId,
        peer_id: payload.peerId,
        peer_name: payload.deviceName,
        pin: null,
        state: 'Initiated',
        is_initiator: false,
        created_at: new Date().toISOString(),
      });
    });

    eventManager.on('pairingPin', (payload) => {
      store.update('activePairingSession', (session) =>
        session
          ? {
              ...session,
              pin: payload.pin,
              peer_name: payload.peerDeviceName,
              state: 'AwaitingPinConfirmation',
            }
          : null,
      );
      store.set('pairingModalMode', 'confirm');
    });

    eventManager.on('pairingComplete', (payload) => {
      store.set('showPairingModal', false);
      store.set('activePairingSession', null);
      store.addToast(`Paired with ${payload.deviceName}!`, 'success');
      // Remove the newly paired peer from discovered list immediately
      store.removeDiscoveredPeer(payload.peerId);
      // Reload both lists to ensure consistency
      this.loadPairedPeers();
    });

    eventManager.on('pairingFailed', (payload) => {
      store.addToast(`Pairing failed: ${payload.error}`, 'error');
      store.set('showPairingModal', false);
      store.set('activePairingSession', null);
    });

    eventManager.on('networkError', (error) => {
      store.addToast(`Network error: ${error}`, 'error');
    });

    // Handle app minimized to tray (desktop only)
    eventManager.on('appMinimizedToTray', () => {
      store.set('isWindowVisible', false);
      store.set('isMinimizedToTray', true);

      // Show first-time native OS notification
      if (!localStorage.getItem('hasShownTrayNotification')) {
        notifyMinimizedToTray();
        localStorage.setItem('hasShownTrayNotification', 'true');
      }
    });

    // Handle clipboard synced from background (Android only)
    eventManager.on('clipboardSyncedFromBackground', (payload) => {
      store.addToast(`Clipboard synced from ${payload.fromDevice}`, 'success');
    });

    // Handle vault status changes - load data when unlocked
    eventManager.on('vaultStatus', async (status) => {
      const previousStatus = store.get('vaultStatus');
      store.set('vaultStatus', status);

      // If transitioning to Unlocked, load app data
      if (status === 'Unlocked' && previousStatus !== 'Unlocked') {
        await this.loadDataAfterUnlock();
      }
    });

    // Handle settings changes from system tray
    eventManager.on('settingsChanged', (payload) => {
      if (payload.auto_sync_enabled !== undefined) {
        const settings = { ...store.get('settings'), auto_sync_enabled: payload.auto_sync_enabled };
        store.set('settings', settings);
      }
    });
  }

  /**
   * Load app data after vault unlock or setup completion.
   * Called when vaultStatus transitions to 'Unlocked'.
   */
  private async loadDataAfterUnlock(): Promise<void> {
    store.set('isLoading', true);
    try {
      await this.loadInitialData();
      store.addToast('Data loaded successfully', 'success');
    } catch (error) {
      console.error('Failed to load data after unlock:', error);
      store.addToast('Failed to load some data', 'error');
      store.set('isLoading', false);
    }
  }

  private async loadInitialData(): Promise<void> {
    try {
      const [deviceInfo, settings, pairedPeers, discoveredPeers, clipboardHistory] = await Promise.all([
        commands.getDeviceInfo(),
        commands.getSettings(),
        commands.getPairedPeers(),
        commands.getDiscoveredPeers(),
        commands.getClipboardHistory(),
      ]);

      store.set('deviceInfo', deviceInfo);
      store.set('settings', settings);
      store.set('pairedPeers', pairedPeers);
      store.set('discoveredPeers', discoveredPeers);
      store.set('clipboardHistory', clipboardHistory);
    } catch (error) {
      console.error('Failed to load initial data:', error);
      store.addToast('Failed to load app data', 'error');
    } finally {
      store.set('isLoading', false);
    }
  }

  private async loadPairedPeers(): Promise<void> {
    const peers = await commands.getPairedPeers();
    store.set('pairedPeers', peers);
  }

  private setupStateSubscriptions(): void {
    store.subscribe('currentView', () => this.render());
    store.subscribe('discoveredPeers', () => this.renderPeersList());
    store.subscribe('pairedPeers', () => this.renderPeersList());
    store.subscribe('clipboardHistory', () => this.renderClipboardHistory());
    store.subscribe('toasts', () => this.renderToasts());
    store.subscribe('showPairingModal', () => this.renderPairingModal());
    store.subscribe('pairingModalMode', () => this.renderPairingModal());
    store.subscribe('activePairingSession', () => this.renderPairingModal());
    store.subscribe('isLoading', () => this.render());
    store.subscribe('vaultStatus', () => this.render());
    store.subscribe('onboardingStep', () => this.render());
    store.subscribe('showResetConfirmation', () => this.render());
    store.subscribe('updateStatus', () => {
      this.renderUpdateBadge();
      this.renderUpdateSection();
    });
    store.subscribe('updateProgress', () => this.renderUpdateSection());
    // Re-render when settings change (for sync toggle and hide clipboard toggle)
    store.subscribe('settings', () => this.render());
  }

  private render(): void {
    const state = store.getState();

    if (state.isLoading) {
      this.root.innerHTML = `
        <div class="flex items-center justify-center h-screen" style="background: #0a0a0b;">
          <!-- Ambient orbs -->
          <div class="orb orb-teal animate-float" style="width: 300px; height: 300px; top: -10%; left: -5%;"></div>
          <div class="orb orb-orange animate-float-delayed" style="width: 250px; height: 250px; bottom: 0; right: -10%;"></div>

          <div class="text-center relative z-10">
            <div class="inline-block mb-6">
              ${icon('loader', 48, 'text-teal-400')}
            </div>
            <p class="text-white/50 text-sm font-medium tracking-wide">Loading DecentPaste...</p>
          </div>
        </div>
      `;
      return;
    }

    // Show onboarding if vault is not set up
    if (state.vaultStatus === 'NotSetup') {
      this.root.innerHTML = this.renderOnboarding();
      return;
    }

    // Show lock screen if vault is locked
    if (state.vaultStatus === 'Locked') {
      this.root.innerHTML = this.renderLockScreen();
      return;
    }

    this.root.innerHTML = `
      <div class="flex flex-col h-screen relative" style="background: #0a0a0b;">
        <!-- Ambient background orbs -->
        <div class="orb orb-teal animate-float" style="width: 400px; height: 400px; top: -15%; left: -10%;"></div>
        <div class="orb orb-orange animate-float-delayed" style="width: 300px; height: 300px; bottom: 10%; right: -15%;"></div>

        <!-- Header -->
        <header class="relative z-10 px-4 py-1 pt-safe-top border-b" style="background: rgba(17, 17, 19, 0.8); backdrop-filter: blur(12px); border-color: rgba(255, 255, 255, 0.06);">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3">
              <img src="${logoDark}" alt="DecentPaste Logo" class="w-12 h-12" />
              <div>
                <h1 class="font-semibold text-white text-sm tracking-tight">DecentPaste</h1>
                <p class="text-xs text-white/40">${state.deviceInfo?.device_name || 'Loading...'}</p>
              </div>
            </div>
            <!-- Lock Button with teal icon container styling -->
            <button id="btn-lock-vault" class="icon-container-teal hover:scale-105 transition-transform" style="width: 2.25rem; height: 2.25rem;" title="Lock vault">
              ${icon('lock', 16)}
            </button>
          </div>
        </header>

        <!-- Main Content -->
        <main class="flex-1 overflow-hidden relative z-10">
          ${this.renderCurrentView()}
        </main>

        <!-- Bottom Navigation -->
        <nav class="relative z-10 pb-safe-bottom" style="background: rgba(17, 17, 19, 0.9); backdrop-filter: blur(12px); border-top: 1px solid rgba(255, 255, 255, 0.06);">
          <div class="flex justify-around py-2">
            ${this.renderNavItem('dashboard', 'home', 'Home')}
            ${this.renderNavItem('peers', 'users', 'Peers')}
            ${this.renderNavItem('settings', 'settings', 'Settings')}
          </div>
        </nav>

        <!-- Toast Container -->
        <div id="toast-container" class="fixed bottom-20 left-4 right-4 flex flex-col gap-2 z-50">
          ${this.renderToastsContent()}
        </div>

        <!-- Pairing Modal -->
        <div id="pairing-modal" class="${state.showPairingModal ? '' : 'hidden'}">
          ${this.renderPairingModalContent()}
        </div>
      </div>
    `;
  }

  private renderNavItem(view: View, iconName: IconName, label: string): string {
    const currentView = store.get('currentView');
    const isActive = currentView === view;
    const updateStatus = store.get('updateStatus');
    const showBadge = view === 'settings' && updateStatus === 'available';

    return `
      <button
        data-nav="${view}"
        class="nav-item ${isActive ? 'nav-item-active' : ''}"
      >
        <div class="relative">
          ${icon(iconName, 20)}
          ${view === 'settings' ? `<span id="nav-settings-badge" class="${showBadge ? 'update-badge' : 'hidden'}"></span>` : ''}
        </div>
        <span>${label}</span>
      </button>
    `;
  }

  private renderCurrentView(): string {
    const view = store.get('currentView');

    switch (view) {
      case 'dashboard':
        return this.renderDashboard();
      case 'peers':
        return this.renderPeersView();
      case 'settings':
        return this.renderSettingsView();
      default:
        return this.renderDashboard();
    }
  }

  private renderDashboard(): string {
    const state = store.getState();
    const historyCount = state.clipboardHistory.length;
    const allItems = state.clipboardHistory;
    const syncEnabled = state.settings.auto_sync_enabled;
    const hideContent = state.settings.hide_clipboard_content;
    const autoLockMinutes = state.settings.auto_lock_minutes;

    return `
      <div class="flex flex-col h-full">
        <!-- Sticky Top Section -->
        <div class="flex-shrink-0 p-4 pb-0">
          <!-- Stats Grid -->
          <div class="grid grid-cols-2 gap-3 mb-6">
            <!-- Sync Toggle Card -->
            <div class="card p-4">
              <div class="flex items-center gap-3">
                <div class="${syncEnabled ? 'icon-container-teal' : 'icon-container-orange'}">
                  ${icon(syncEnabled ? 'refreshCw' : 'wifiOff', 18)}
                </div>
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-medium text-white">Sync</p>
                  <p class="text-xs text-white/40">${syncEnabled ? 'Active' : 'Paused'}</p>
                </div>
                <label class="relative inline-block w-10 h-6 flex-shrink-0">
                  <input
                    type="checkbox"
                    id="dashboard-sync-toggle"
                    ${syncEnabled ? 'checked' : ''}
                    class="peer sr-only"
                  />
                  <span class="absolute inset-0 bg-white/10 rounded-full cursor-pointer transition-colors peer-checked:bg-teal-500/50"></span>
                  <span class="absolute left-1 top-1 w-4 h-4 bg-white/60 rounded-full transition-transform peer-checked:translate-x-4 peer-checked:bg-white"></span>
                </label>
              </div>
            </div>
            <!-- Clipboard Count Card -->
            <div class="card p-4">
              <div class="flex items-center gap-3">
                <div class="icon-container-teal">
                  ${icon('clipboard', 18)}
                </div>
                <div>
                  <p id="clipboard-count" class="text-2xl font-bold text-white tracking-tight">${historyCount}</p>
                  <p class="text-xs text-white/40">Clipboard Items</p>
                </div>
              </div>
            </div>
          </div>

          <!-- Quick Actions -->
          <div class="mb-6">
            <h2 class="text-sm font-semibold text-white/80 mb-3 tracking-tight">Quick Actions</h2>
            <div class="flex gap-3">
              <button id="btn-share-clipboard" class="btn-primary flex-1">
                ${icon('share', 18)}
                <span>Share Clipboard</span>
              </button>
              <div class="flex items-center gap-2">
                <span class="text-white/40">${icon('lock', 14)}</span>
                <select id="auto-lock-select" class="bg-white/5 border border-white/10 rounded-lg px-2 py-2 text-xs text-white/70 cursor-pointer hover:bg-white/10 transition-colors focus:outline-none focus:ring-1 focus:ring-teal-500/50">
                  <option value="0" ${autoLockMinutes === 0 ? 'selected' : ''}>Never</option>
                  <option value="1" ${autoLockMinutes === 1 ? 'selected' : ''}>1 min</option>
                  <option value="5" ${autoLockMinutes === 5 ? 'selected' : ''}>5 min</option>
                  <option value="15" ${autoLockMinutes === 15 ? 'selected' : ''}>15 min</option>
                  <option value="30" ${autoLockMinutes === 30 ? 'selected' : ''}>30 min</option>
                  <option value="60" ${autoLockMinutes === 60 ? 'selected' : ''}>1 hour</option>
                </select>
              </div>
            </div>
          </div>

          <!-- Clipboard History Header -->
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Clipboard History</h2>
            <div class="flex items-center gap-2">
              <button id="btn-toggle-visibility" class="p-1.5 rounded-lg text-white/40 hover:text-teal-400 hover:bg-teal-500/10 transition-all" title="${hideContent ? 'Show content' : 'Hide content'}">
                ${icon(hideContent ? 'eye' : 'eyeOff', 16)}
              </button>
              <button id="btn-clear-history" class="btn-danger text-xs px-3 py-1.5">
                ${icon('trash', 12)}
                <span>Clear All</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Scrollable Clipboard History (Full List) -->
        <div class="flex-1 min-h-0 overflow-y-auto px-4 pb-4">
          <div id="clipboard-history-list" class="space-y-2">
            ${allItems.length > 0 ? allItems.map((item) => this.renderClipboardItem(item, hideContent)).join('') : this.renderEmptyState('No clipboard items yet', 'Copy something to get started')}
          </div>
        </div>
      </div>
    `;
  }

  private renderPeersView(): string {
    const state = store.getState();
    const pairedPeers = state.pairedPeers;
    const discoveredPeers = state.discoveredPeers.filter((d) => !pairedPeers.some((p) => p.peer_id === d.peer_id));

    return `
      <div class="p-4 h-full overflow-y-auto">
        <!-- Paired Devices -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-green" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('link', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Paired Devices</h2>
            <span class="text-xs text-white/30 ml-auto">${pairedPeers.length}</span>
          </div>
          <div id="paired-peers" class="space-y-2">
            ${pairedPeers.length > 0 ? pairedPeers.map((peer) => this.renderPairedPeer(peer)).join('') : this.renderEmptyState('No paired devices', 'Pair with a device below')}
          </div>
        </div>

        <!-- Discovered Devices -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-orange" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('wifi', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Discovered Devices</h2>
            <span class="text-xs text-white/30 ml-auto mr-2">${discoveredPeers.length}</span>
            <button id="btn-refresh-peers" class="p-1.5 rounded-lg text-white/40 hover:text-teal-400 hover:bg-teal-500/10 transition-all" title="Refresh">
              ${icon('refreshCw', 14)}
            </button>
          </div>
          <div id="discovered-peers" class="space-y-2">
            ${discoveredPeers.length > 0 ? discoveredPeers.map((peer) => this.renderDiscoveredPeer(peer)).join('') : this.renderEmptyState('No devices found', 'Searching on local network...')}
          </div>
        </div>
      </div>
    `;
  }

  private renderSettingsView(): string {
    const settings = store.get('settings');
    const deviceInfo = store.get('deviceInfo');

    return `
      <div class="p-4 h-full overflow-y-auto">
        <!-- Device Info -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-purple" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('monitor', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Device</h2>
          </div>
          <div class="card p-4">
            <div class="flex items-center gap-3">
              <div class="icon-container-teal icon-container-lg">
                ${icon('monitor', 24)}
              </div>
              <div class="flex-1 min-w-0">
                <input
                  id="device-name-input"
                  type="text"
                  value="${settings.device_name}"
                  class="input w-full font-semibold text-white"
                  style="padding: 0.5rem 0.75rem;"
                />
                <p class="text-xs text-white/30 mt-1 font-mono">${deviceInfo?.device_id?.slice(0, 16) || 'Unknown'}...</p>
              </div>
            </div>
          </div>
        </div>

        <!-- Sync Settings -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-green" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('refreshCw', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Sync</h2>
          </div>
          <div class="card overflow-hidden">
            <label class="flex items-center justify-between p-4 cursor-pointer hover:bg-white/[0.02] transition-colors">
              <span class="text-sm text-white/70">Auto-sync clipboard</span>
              <input
                type="checkbox"
                id="auto-sync-toggle"
                ${settings.auto_sync_enabled ? 'checked' : ''}
                class="checkbox"
              />
            </label>
            <div class="divider"></div>
            <label class="flex items-center justify-between p-4 cursor-pointer hover:bg-white/[0.02] transition-colors">
              <span class="text-sm text-white/70">Show notifications</span>
              <input
                type="checkbox"
                id="notifications-toggle"
                ${settings.show_notifications ? 'checked' : ''}
                class="checkbox"
              />
            </label>
          </div>
        </div>

        <!-- History Settings -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-blue" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('history', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">History</h2>
          </div>
          <div class="card overflow-hidden">
            <div class="flex items-center justify-between p-4">
              <span class="text-sm text-white/70">History limit</span>
              <select id="history-limit-select" class="select">
                <option value="25" ${settings.clipboard_history_limit === 25 ? 'selected' : ''}>25 items</option>
                <option value="50" ${settings.clipboard_history_limit === 50 ? 'selected' : ''}>50 items</option>
                <option value="100" ${settings.clipboard_history_limit === 100 ? 'selected' : ''}>100 items</option>
              </select>
            </div>
            <div class="divider"></div>
            <label class="flex items-center justify-between p-4 cursor-pointer hover:bg-white/[0.02] transition-colors">
              <div>
                <span class="text-sm text-white/70 block">Keep clipboard history</span>
                <span class="text-xs text-white/40">Save history in encrypted vault</span>
              </div>
              <input
                type="checkbox"
                id="keep-history-toggle"
                ${settings.keep_history ? 'checked' : ''}
                class="checkbox"
              />
            </label>
          </div>
        </div>

        <!-- Security -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-teal" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('lock', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Security</h2>
          </div>
          <div class="card overflow-hidden">
            <div class="flex items-center justify-between p-4">
              <div>
                <span class="text-sm text-white/70 block">Auto-lock</span>
                <span class="text-xs text-white/40">Lock vault after inactivity</span>
              </div>
              <select id="auto-lock-select" class="select">
                <option value="0" ${settings.auto_lock_minutes === 0 ? 'selected' : ''}>Never</option>
                <option value="1" ${settings.auto_lock_minutes === 1 ? 'selected' : ''}>1 minute</option>
                <option value="5" ${settings.auto_lock_minutes === 5 ? 'selected' : ''}>5 minutes</option>
                <option value="15" ${settings.auto_lock_minutes === 15 ? 'selected' : ''}>15 minutes</option>
                <option value="30" ${settings.auto_lock_minutes === 30 ? 'selected' : ''}>30 minutes</option>
                <option value="60" ${settings.auto_lock_minutes === 60 ? 'selected' : ''}>1 hour</option>
              </select>
            </div>
          </div>
        </div>

        <!-- Updates -->
        <div class="mb-6">
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-blue" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('download', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Updates</h2>
          </div>
          <div id="update-section" class="card overflow-hidden">
            ${this.renderUpdateContent()}
          </div>
        </div>

        <!-- About -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <div class="icon-container-orange" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('clipboard', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">About</h2>
          </div>
          <div class="card p-4">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 rounded-xl flex items-center justify-center glow-teal" style="background: linear-gradient(135deg, #14b8a6 0%, #0d9488 100%);">
                ${icon('clipboard', 20, 'text-white')}
              </div>
              <div>
                <p class="text-sm font-semibold text-white">DecentPaste <span class="text-white/40 font-normal">v0.1.0</span></p>
                <p class="text-xs text-white/40">Cross-platform P2P clipboard sharing</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  /**
   * Renders the lock screen for returning users.
   * Shows PIN input with masked digits.
   */
  private renderLockScreen(): string {
    const settings = store.get('settings');
    const showResetConfirmation = store.get('showResetConfirmation');
    const deviceName = settings.device_name || 'Your Device';

    return `
      <div class="flex flex-col h-screen relative" style="background: #0a0a0b;">
        <!-- Ambient background orbs -->
        <div class="orb orb-teal animate-float" style="width: 400px; height: 400px; top: -15%; left: -10%;"></div>
        <div class="orb orb-orange animate-float-delayed" style="width: 300px; height: 300px; bottom: 10%; right: -15%;"></div>

        <!-- Lock Screen Content -->
        <div class="flex-1 flex flex-col items-center justify-center relative z-10 p-6 pt-safe-top pb-safe-bottom">
          <!-- Logo and Device Name -->
          <div class="text-center mb-8">
            <div class="w-20 h-20 rounded-2xl mx-auto mb-4 flex items-center justify-center glow-teal" style="background: linear-gradient(135deg, rgba(20, 184, 166, 0.2) 0%, rgba(13, 148, 136, 0.1) 100%); border: 1px solid rgba(20, 184, 166, 0.3);">
              ${icon('lock', 36, 'text-teal-400')}
            </div>
            <h1 class="text-xl font-semibold text-white mb-1">Welcome back</h1>
            <p class="text-white/50 text-sm">${escapeHtml(deviceName)}</p>
          </div>

          <!-- PIN Input -->
          <div class="w-full max-w-xs mb-6">
            <label class="block text-sm text-white/60 mb-2 text-center">Enter your PIN to unlock</label>
            <div class="relative">
              <input
                type="password"
                id="lock-pin-input"
                inputmode="numeric"
                pattern="[0-9]*"
                maxlength="8"
                placeholder="••••"
                autocomplete="off"
                class="w-full px-4 py-3 rounded-xl text-center text-xl tracking-[0.5em] font-mono"
                style="background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); color: white; outline: none;"
              />
            </div>
            <p id="lock-error" class="text-red-400 text-xs text-center mt-2 hidden"></p>
          </div>

          <!-- Unlock Button -->
          <button id="btn-unlock" class="btn-primary w-full max-w-xs mb-4">
            ${icon('unlock', 18)}
            <span>Unlock</span>
          </button>

          <!-- Forgot PIN Link -->
          <button id="btn-forgot-pin" class="text-white/40 hover:text-white/60 text-sm transition-colors">
            Forgot PIN?
          </button>
        </div>

        <!-- Toast Container -->
        <div id="toast-container" class="fixed bottom-20 left-4 right-4 flex flex-col gap-2 z-50">
          ${this.renderToastsContent()}
        </div>

        <!-- Reset Confirmation Modal -->
        ${showResetConfirmation ? this.renderResetConfirmation() : ''}
      </div>
    `;
  }

  /**
   * Renders the onboarding wizard for first-time setup.
   * 2-step flow: Device Name → PIN Setup
   */
  private renderOnboarding(): string {
    const step = store.get('onboardingStep') || 'device-name';
    const deviceInfo = store.get('deviceInfo');
    const defaultDeviceName = deviceInfo?.device_name || 'My Device';

    // Progress indicator
    const stepNumber = step === 'device-name' ? 1 : 2;
    const progressIndicator = `
      <div class="flex items-center justify-center gap-2 mb-8">
        <div class="flex items-center gap-1">
          ${[1, 2]
            .map(
              (n) => `
            <div class="w-2 h-2 rounded-full transition-all ${n === stepNumber ? 'bg-teal-400 w-6' : n < stepNumber ? 'bg-teal-400/50' : 'bg-white/20'}"></div>
          `,
            )
            .join('')}
        </div>
        <span class="text-xs text-white/40 ml-2">Step ${stepNumber} of 2</span>
      </div>
    `;

    let stepContent = '';

    if (step === 'device-name') {
      const savedDeviceName = store.get('onboardingDeviceName') || defaultDeviceName;
      stepContent = `
        <div class="text-center mb-6">
          <div class="w-16 h-16 rounded-2xl mx-auto mb-4 flex items-center justify-center glow-teal" style="background: linear-gradient(135deg, rgba(20, 184, 166, 0.2) 0%, rgba(13, 148, 136, 0.1) 100%); border: 1px solid rgba(20, 184, 166, 0.3);">
            ${icon('monitor', 28, 'text-teal-400')}
          </div>
          <h2 class="text-xl font-semibold text-white mb-2">Name Your Device</h2>
          <p class="text-white/50 text-sm">This name will be visible to other devices on your network</p>
        </div>

        <div class="mb-6">
          <label class="block text-sm text-white/60 mb-2">Device Name</label>
          <input
            type="text"
            id="onboarding-device-name"
            value="${escapeHtml(savedDeviceName)}"
            maxlength="50"
            placeholder="e.g., My MacBook"
            class="w-full px-4 py-3 rounded-xl text-white"
            style="background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); outline: none;"
          />
          <p id="onboarding-step1-error" class="text-red-400 text-xs mt-2 hidden"></p>
        </div>

        <button id="btn-onboarding-step1-continue" class="btn-primary w-full">
          ${icon('arrowRight', 18)}
          <span>Continue</span>
        </button>
      `;
    } else if (step === 'pin-setup') {
      stepContent = `
        <div class="text-center mb-6">
          <div class="w-16 h-16 rounded-2xl mx-auto mb-4 flex items-center justify-center" style="background: linear-gradient(135deg, rgba(20, 184, 166, 0.2) 0%, rgba(13, 148, 136, 0.1) 100%); border: 1px solid rgba(20, 184, 166, 0.3);">
            ${icon('key', 28, 'text-teal-400')}
          </div>
          <h2 class="text-xl font-semibold text-white mb-2">Create Your PIN</h2>
          <p class="text-white/50 text-sm">Choose a 4-8 digit PIN to secure your vault</p>
        </div>

        <div class="space-y-4 mb-6">
          <div>
            <label class="block text-sm text-white/60 mb-2">Enter PIN</label>
            <input
              type="password"
              id="onboarding-pin"
              inputmode="numeric"
              pattern="[0-9]*"
              maxlength="8"
              placeholder="••••"
              autocomplete="new-password"
              class="w-full px-4 py-3 rounded-xl text-center text-xl tracking-[0.5em] font-mono"
              style="background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); color: white; outline: none;"
            />
          </div>
          <div>
            <label class="block text-sm text-white/60 mb-2">Confirm PIN</label>
            <input
              type="password"
              id="onboarding-pin-confirm"
              inputmode="numeric"
              pattern="[0-9]*"
              maxlength="8"
              placeholder="••••"
              autocomplete="new-password"
              class="w-full px-4 py-3 rounded-xl text-center text-xl tracking-[0.5em] font-mono"
              style="background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); color: white; outline: none;"
            />
          </div>
          <p id="onboarding-step3-error" class="text-red-400 text-xs text-center hidden"></p>
        </div>

        <div class="space-y-3">
          <button id="btn-onboarding-complete" class="btn-primary w-full">
            ${icon('check', 18)}
            <span>Complete Setup</span>
          </button>
          <button id="btn-onboarding-step2-back" class="btn-secondary w-full">
            ${icon('arrowLeft', 18)}
            <span>Back</span>
          </button>
        </div>
      `;
    }

    return `
      <div class="flex flex-col h-screen relative" style="background: #0a0a0b;">
        <!-- Ambient background orbs -->
        <div class="orb orb-teal animate-float" style="width: 400px; height: 400px; top: -15%; left: -10%;"></div>
        <div class="orb orb-orange animate-float-delayed" style="width: 300px; height: 300px; bottom: 10%; right: -15%;"></div>

        <!-- Onboarding Content -->
        <div class="flex-1 flex flex-col items-center justify-center relative z-10 p-6 pt-safe-top pb-safe-bottom">
          <div class="w-full max-w-sm">
            <!-- Logo -->
            <div class="text-center mb-6">
              <img src="${logoDark}" alt="DecentPaste Logo" class="w-16 h-16 mx-auto mb-2" />
              <h1 class="text-lg font-semibold text-white tracking-tight">Welcome to DecentPaste</h1>
            </div>

            ${progressIndicator}
            ${stepContent}
          </div>
        </div>

        <!-- Toast Container -->
        <div id="toast-container" class="fixed bottom-20 left-4 right-4 flex flex-col gap-2 z-50">
          ${this.renderToastsContent()}
        </div>
      </div>
    `;
  }

  /**
   * Renders the reset confirmation modal.
   * Requires user to type "RESET" to confirm destructive action.
   */
  private renderResetConfirmation(): string {
    return `
      <div class="fixed inset-0 modal-overlay flex items-center justify-center z-50 p-4">
        <div class="modal-content p-6 max-w-sm w-full">
          <div class="text-center">
            <!-- Warning Icon -->
            <div class="w-16 h-16 rounded-2xl mx-auto mb-4 flex items-center justify-center" style="background: linear-gradient(135deg, rgba(239, 68, 68, 0.2) 0%, rgba(185, 28, 28, 0.1) 100%); border: 1px solid rgba(239, 68, 68, 0.3);">
              ${icon('alertTriangle', 28, 'text-red-400')}
            </div>

            <h2 class="text-xl font-semibold text-white mb-2">Reset Vault?</h2>
            <p class="text-white/50 text-sm mb-4">This will permanently delete all your data:</p>

            <!-- Warning List -->
            <div class="text-left mb-6 p-3 rounded-xl" style="background: rgba(239, 68, 68, 0.1); border: 1px solid rgba(239, 68, 68, 0.2);">
              <ul class="text-sm text-red-300/80 space-y-1">
                <li class="flex items-center gap-2">
                  ${icon('x', 14, 'text-red-400')}
                  <span>Paired devices will be unpaired</span>
                </li>
                <li class="flex items-center gap-2">
                  ${icon('x', 14, 'text-red-400')}
                  <span>Clipboard history will be erased</span>
                </li>
                <li class="flex items-center gap-2">
                  ${icon('x', 14, 'text-red-400')}
                  <span>Encryption keys will be destroyed</span>
                </li>
              </ul>
            </div>

            <!-- Confirmation Input -->
            <div class="mb-6">
              <label class="block text-sm text-white/60 mb-2">Type <span class="font-mono font-bold text-red-400">RESET</span> to confirm</label>
              <input
                type="text"
                id="reset-confirmation-input"
                placeholder="Type RESET here"
                autocomplete="off"
                autocapitalize="characters"
                class="w-full px-4 py-3 rounded-xl text-center text-white font-mono uppercase"
                style="background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); outline: none;"
              />
              <p id="reset-error" class="text-red-400 text-xs text-center mt-2 hidden"></p>
            </div>

            <!-- Buttons -->
            <div class="space-y-3">
              <button id="btn-reset-confirm" class="btn-danger w-full">
                ${icon('trash', 18)}
                <span>Reset Everything</span>
              </button>
              <button id="btn-reset-cancel" class="btn-secondary w-full">
                ${icon('arrowLeft', 18)}
                <span>Cancel</span>
              </button>
            </div>
          </div>
        </div>
      </div>
    `;
  }

  private renderClipboardItem(item: ClipboardEntry, hideContent = false): string {
    const isLocal = item.is_local;
    // Escape HTML to prevent XSS attacks from malicious clipboard content
    const safeContent = hideContent ? '••••••••••••••••' : escapeHtml(truncate(item.content, 120));
    return `
      <div class="card p-3 group cursor-pointer clipboard-item" data-id="${item.id}" style="transition: all 0.2s ease;">
        <div class="flex items-start justify-between gap-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm text-white/90 break-words line-clamp-2 leading-relaxed ${hideContent ? 'select-none' : ''}">${safeContent}</p>
            <div class="flex items-center gap-2 mt-2">
              <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium ${isLocal ? 'bg-teal-500/10 text-teal-400' : 'bg-orange-500/10 text-orange-400'}">
                ${isLocal ? icon('monitor', 10) : icon('download', 10)}
                ${isLocal ? 'Local' : escapeHtml(item.origin_device_name)}
              </span>
              <span class="text-xs text-white/30">${formatTime(item.timestamp)}</span>
            </div>
          </div>
          <button
            data-copy="${item.id}"
            class="copy-btn p-2 rounded-lg text-white/30 hover:text-teal-400 hover:bg-teal-500/10 transition-all flex-shrink-0"
            title="Copy to clipboard"
          >
            ${icon('copy', 16)}
          </button>
        </div>
      </div>
    `;
  }

  private renderPairedPeer(peer: PairedPeer): string {
    const safeName = escapeHtml(peer.device_name);
    return `
      <div class="card p-3 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="icon-container-green">
            ${icon('monitor', 18)}
          </div>
          <p class="text-sm font-medium text-white">${safeName}</p>
        </div>
        <button
          data-unpair="${peer.peer_id}"
          class="p-2 rounded-lg text-white/30 hover:text-red-400 hover:bg-red-500/10 transition-all"
          title="Unpair device"
        >
          ${icon('unlink', 16)}
        </button>
      </div>
    `;
  }

  private renderDiscoveredPeer(peer: DiscoveredPeer): string {
    const safeName = peer.device_name ? escapeHtml(peer.device_name) : 'Unknown Device';
    return `
      <div class="card p-3 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="icon-container-orange">
            ${icon('smartphone', 18)}
          </div>
          <div>
            <p class="text-sm font-medium text-white">${safeName}</p>
            <p class="text-xs text-white/40">Discovered ${formatTime(peer.discovered_at)}</p>
          </div>
        </div>
        <button
          data-pair="${peer.peer_id}"
          class="btn-primary text-xs px-4 py-1.5"
        >
          Pair
        </button>
      </div>
    `;
  }

  private renderEmptyState(title: string, subtitle: string): string {
    return `
      <div class="text-center py-12">
        <div class="w-16 h-16 rounded-2xl mx-auto mb-4 flex items-center justify-center" style="background: rgba(255, 255, 255, 0.03); border: 1px solid rgba(255, 255, 255, 0.06);">
          ${icon('clipboard', 24, 'text-white/20')}
        </div>
        <p class="text-white/50 text-sm font-medium">${title}</p>
        <p class="text-white/30 text-xs mt-1">${subtitle}</p>
      </div>
    `;
  }

  private renderToastsContent(): string {
    const toasts = store.get('toasts');
    return toasts.map((toast) => this.renderToast(toast)).join('');
  }

  private renderToasts(): void {
    const container = $('#toast-container');
    if (container) {
      container.innerHTML = this.renderToastsContent();
    }
  }

  private renderToast(toast: Toast): string {
    const toastClass =
      toast.type === 'success' ? 'toast-success' : toast.type === 'error' ? 'toast-error' : 'toast-info';

    return `
      <div class="toast ${toastClass}">
        <div class="flex items-center gap-2">
          ${toast.type === 'success' ? icon('check', 16) : toast.type === 'error' ? icon('x', 16) : icon('clipboard', 16)}
          <span class="text-sm font-medium">${toast.message}</span>
        </div>
        <button data-dismiss-toast="${toast.id}" class="p-1 hover:opacity-60 transition-opacity">
          ${icon('x', 14)}
        </button>
      </div>
    `;
  }

  private renderPairingModalContent(): string {
    const session = store.get('activePairingSession');
    const mode = store.get('pairingModalMode');

    if (!session) return '';

    // Escape peer name to prevent XSS from malicious device names
    const safePeerName = session.peer_name ? escapeHtml(session.peer_name) : null;

    let content = '';

    if (mode === 'respond') {
      content = `
        <div class="text-center">
          <div class="icon-container-teal icon-container-lg mx-auto mb-4" style="width: 4rem; height: 4rem;">
            ${icon('link', 28)}
          </div>
          <h3 class="text-lg font-semibold text-white mb-2 tracking-tight">Pairing Request</h3>
          <p class="text-white/50 mb-6">${safePeerName || 'A device'} wants to pair with you</p>
          <div class="flex gap-3">
            <button id="btn-reject-pairing" class="btn-secondary flex-1" style="touch-action: manipulation">
              Reject
            </button>
            <button id="btn-accept-pairing" class="btn-primary flex-1" style="touch-action: manipulation">
              Accept
            </button>
          </div>
        </div>
      `;
    } else if (mode === 'confirm' && session.pin) {
      const isInitiator = session.is_initiator;
      const pinDigits = session.pin
        .split('')
        .map((d) => `<span class="pin-digit">${d}</span>`)
        .join('');

      const buttonArea = isInitiator
        ? `
          <div class="flex gap-3">
            <button id="btn-cancel-pairing" class="btn-secondary flex-1" style="touch-action: manipulation">
              Cancel
            </button>
            <button id="btn-confirm-pin" class="btn-primary flex-1" style="touch-action: manipulation">
              Confirm
            </button>
          </div>
        `
        : `
          <p class="text-white/40 text-sm mb-4">Waiting for other device to confirm...</p>
          <button id="btn-cancel-pairing" class="btn-secondary" style="touch-action: manipulation">
            Cancel
          </button>
        `;

      content = `
        <div class="text-center">
          <div class="icon-container-green icon-container-lg mx-auto mb-4" style="width: 4rem; height: 4rem;">
            ${icon('check', 28)}
          </div>
          <h3 class="text-lg font-semibold text-white mb-2 tracking-tight">Confirm PIN</h3>
          <p class="text-white/50 mb-6">Verify this PIN matches on both devices</p>
          <div class="pin-display mx-auto mb-6">
            ${pinDigits}
          </div>
          ${buttonArea}
        </div>
      `;
    } else if (mode === 'initiate') {
      content = `
        <div class="text-center">
          <div class="mx-auto mb-4">
            ${icon('loader', 48, 'text-teal-400 animate-spin')}
          </div>
          <h3 class="text-lg font-semibold text-white mb-2 tracking-tight">Pairing...</h3>
          <p class="text-white/50 mb-6">Waiting for ${safePeerName || 'device'} to respond</p>
          <button id="btn-cancel-pairing" class="btn-secondary" style="touch-action: manipulation">
            Cancel
          </button>
        </div>
      `;
    }

    return `
      <div class="fixed inset-0 modal-overlay flex items-center justify-center z-50">
        <div class="modal-content p-6 m-4 max-w-sm w-full">
          ${content}
        </div>
      </div>
    `;
  }

  private renderPairingModal(): void {
    if (this.modalRenderPending) return;
    this.modalRenderPending = true;

    queueMicrotask(() => {
      this.modalRenderPending = false;
      const modal = $('#pairing-modal');
      if (modal) {
        const show = store.get('showPairingModal');
        modal.className = show ? '' : 'hidden';
        modal.innerHTML = this.renderPairingModalContent();
      }
    });
  }

  private renderPeersList(): void {
    const view = store.get('currentView');
    if (view === 'peers') {
      const content = $('main');
      if (content) {
        content.innerHTML = this.renderPeersView();
      }
    }
  }

  private renderClipboardHistory(): void {
    const view = store.get('currentView');
    if (view === 'dashboard') {
      const history = store.get('clipboardHistory');
      const hideContent = store.get('settings').hide_clipboard_content;

      // Update the clipboard count in stats
      const countEl = $('#clipboard-count');
      if (countEl) {
        countEl.textContent = String(history.length);
      }

      // Update the full clipboard history list
      const container = $('#clipboard-history-list');
      if (container) {
        container.innerHTML =
          history.length > 0
            ? history.map((item) => this.renderClipboardItem(item, hideContent)).join('')
            : this.renderEmptyState('No clipboard items yet', 'Copy something to get started');
      }
    }
  }

  private renderUpdateContent(): string {
    const status = store.get('updateStatus');
    const updateInfo = store.get('updateInfo');
    const progress = store.get('updateProgress');
    const error = store.get('updateError');

    switch (status) {
      case 'idle':
        return `
          <div class="flex items-center justify-between p-4">
            <span class="text-sm text-white/70">Check for updates</span>
            <button id="btn-check-update" class="btn-primary text-xs px-4 py-1.5">
              ${icon('refreshCw', 14)}
              <span>Check</span>
            </button>
          </div>
        `;

      case 'checking':
        return `
          <div class="flex items-center justify-between p-4">
            <span class="text-sm text-white/70">Checking for updates...</span>
            <div class="text-teal-400">
              ${icon('loader', 18, 'animate-spin')}
            </div>
          </div>
        `;

      case 'up-to-date':
        return `
          <div class="p-4">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-2">
                ${icon('check', 16, 'text-green-400')}
                <span class="text-sm text-white/70">You're up to date!</span>
              </div>
              <button id="btn-check-update" class="text-xs text-teal-400 hover:text-teal-300 font-medium transition-colors">
                Check again
              </button>
            </div>
            <p class="text-xs text-white/40 mt-1">Current version: v0.1.0</p>
          </div>
        `;

      case 'available':
        return `
          <div class="p-4">
            <div class="flex items-center gap-2 mb-3">
              <div class="w-2 h-2 rounded-full bg-orange-400 animate-pulse"></div>
              <span class="text-sm font-medium text-white">Update available!</span>
            </div>
            <div class="mb-3">
              <p class="text-sm text-white/70">Version ${updateInfo?.version || 'unknown'} is ready</p>
              ${updateInfo?.body ? `<p class="text-xs text-white/40 mt-1 line-clamp-2">${escapeHtml(updateInfo.body)}</p>` : ''}
            </div>
            <button id="btn-download-update" class="btn-primary w-full">
              ${icon('download', 16)}
              <span>Download & Install</span>
            </button>
          </div>
        `;

      case 'downloading':
        const percentage = getDownloadPercentage();
        const downloadedStr = progress ? formatBytes(progress.downloaded) : '0 B';
        const totalStr = progress?.total ? formatBytes(progress.total) : 'unknown';
        return `
          <div class="p-4">
            <div class="flex items-center gap-2 mb-3">
              ${icon('download', 16, 'text-teal-400')}
              <span class="text-sm text-white/70">Downloading update...</span>
            </div>
            <div class="w-full bg-white/10 rounded-full h-2 mb-2">
              <div class="bg-teal-400 h-2 rounded-full transition-all duration-300" style="width: ${percentage}%"></div>
            </div>
            <p class="text-xs text-white/40">${downloadedStr} / ${totalStr} (${percentage}%)</p>
          </div>
        `;

      case 'ready':
        return `
          <div class="p-4">
            <div class="flex items-center gap-2 mb-3">
              ${icon('check', 16, 'text-green-400')}
              <span class="text-sm text-white/70">Update ready to install</span>
            </div>
            <p class="text-xs text-white/40">The app will restart to complete the update.</p>
          </div>
        `;

      case 'installing':
        return `
          <div class="flex items-center justify-between p-4">
            <span class="text-sm text-white/70">Installing update...</span>
            <div class="text-teal-400">
              ${icon('loader', 18, 'animate-spin')}
            </div>
          </div>
        `;

      case 'error':
        return `
          <div class="p-4">
            <div class="flex items-center gap-2 mb-2">
              ${icon('x', 16, 'text-red-400')}
              <span class="text-sm text-white/70">Update check failed</span>
            </div>
            <p class="text-xs text-red-400/70 mb-3">${error ? escapeHtml(error) : 'Unknown error'}</p>
            <button id="btn-check-update" class="btn-secondary text-xs px-4 py-1.5">
              ${icon('refreshCw', 14)}
              <span>Retry</span>
            </button>
          </div>
        `;

      default:
        return `
          <div class="flex items-center justify-between p-4">
            <span class="text-sm text-white/70">Check for updates</span>
            <button id="btn-check-update" class="btn-primary text-xs px-4 py-1.5">
              ${icon('refreshCw', 14)}
              <span>Check</span>
            </button>
          </div>
        `;
    }
  }

  private renderUpdateSection(): void {
    const view = store.get('currentView');
    if (view === 'settings') {
      const container = $('#update-section');
      if (container) {
        container.innerHTML = this.renderUpdateContent();
      }
    }
  }

  private renderUpdateBadge(): void {
    const status = store.get('updateStatus');
    const badge = $('#nav-settings-badge');
    if (badge) {
      badge.className = status === 'available' ? 'update-badge' : 'hidden';
    }
  }

  /**
   * Reset the auto-lock timer based on current settings.
   * Called on user activity and when settings change.
   */
  private resetAutoLockTimer(): void {
    // Clear existing timer
    if (this.autoLockTimer) {
      clearTimeout(this.autoLockTimer);
      this.autoLockTimer = null;
    }

    const settings = store.get('settings');
    const vaultStatus = store.get('vaultStatus');

    // Only set timer if auto-lock is enabled and vault is unlocked
    if (settings.auto_lock_minutes > 0 && vaultStatus === 'Unlocked') {
      const timeoutMs = settings.auto_lock_minutes * 60 * 1000;
      this.autoLockTimer = setTimeout(async () => {
        try {
          await commands.lockVault();
          store.addToast('Vault locked due to inactivity', 'info');
        } catch (error) {
          console.error('Failed to auto-lock vault:', error);
        }
      }, timeoutMs);
    }
  }

  /**
   * Setup activity tracking to reset the auto-lock timer.
   * Listens for user interactions to detect activity.
   */
  private setupActivityTracking(): void {
    const activityEvents = ['mousedown', 'keydown', 'touchstart', 'scroll'];
    activityEvents.forEach((eventType) => {
      document.addEventListener(eventType, () => this.resetAutoLockTimer(), { passive: true });
    });

    // Also reset timer when vault is unlocked or settings change
    store.subscribe('vaultStatus', () => this.resetAutoLockTimer());
    store.subscribe('settings', () => this.resetAutoLockTimer());
  }
}

export async function initApp(rootElement: HTMLElement): Promise<void> {
  const app = new App(rootElement);
  await app.init();
}
