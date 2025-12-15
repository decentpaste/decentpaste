import { store, type Toast, type View } from './state/store';
import { eventManager } from './api/events';
import * as commands from './api/commands';
import { readText } from '@tauri-apps/plugin-clipboard-manager';
import { icon, type IconName } from './components/icons';
import { $, escapeHtml, formatTime, truncate } from './utils/dom';
import { getErrorMessage } from './utils/error';
import { notifyClipboardReceived, notifyMinimizedToTray } from './utils/notifications';
import type { ClipboardEntry, DiscoveredPeer, PairedPeer } from './api/types';
import logoDark from './assets/logo_dark.svg';

class App {
  private root: HTMLElement;
  private pairingInProgress: boolean = false; // Guard against duplicate pairing operations
  private modalRenderPending: boolean = false; // Debounce modal renders

  constructor(rootElement: HTMLElement) {
    this.root = rootElement;
  }

  async init(): Promise<void> {
    // Setup event listeners from backend
    await eventManager.setup();
    this.setupEventHandlers();

    // Load initial data
    await this.loadInitialData();

    // Render UI
    this.render();

    // Setup delegated event listeners (once, on root)
    this.setupDelegatedListeners();

    // Subscribe to state changes
    this.setupStateSubscriptions();
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

      // Copy buttons
      const copyEl = target.closest('[data-copy]');
      if (copyEl) {
        const id = copyEl.getAttribute('data-copy');
        const history = store.get('clipboardHistory');
        const item = history.find((h) => h.id === id);
        if (item) {
          await commands.setClipboard(item.content);
          store.addToast('Copied to clipboard', 'success');
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
    });

    // Handle change events for settings (needs separate listener due to event type)
    this.root.addEventListener('change', async (e) => {
      const target = e.target as HTMLInputElement | HTMLSelectElement;

      // Auto-sync toggle
      if (target.id === 'auto-sync-toggle') {
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

      // Clear on exit toggle
      if (target.id === 'clear-on-exit-toggle') {
        const checked = (target as HTMLInputElement).checked;
        const settings = { ...store.get('settings'), clear_history_on_exit: checked };
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

    this.root.innerHTML = `
      <div class="flex flex-col h-screen relative" style="background: #0a0a0b;">
        <!-- Ambient background orbs -->
        <div class="orb orb-teal animate-float" style="width: 400px; height: 400px; top: -15%; left: -10%;"></div>
        <div class="orb orb-orange animate-float-delayed" style="width: 300px; height: 300px; bottom: 10%; right: -15%;"></div>

        <!-- Header -->
        <header class="relative z-10 px-4 py-1 pt-safe-top border-b" style="background: rgba(17, 17, 19, 0.8); backdrop-filter: blur(12px); border-color: rgba(255, 255, 255, 0.06);">
          <div class="flex items-center">
            <div class="flex items-center gap-3">
              <img src="${logoDark}" alt="DecentPaste Logo" class="w-12 h-12" />
              <div>
                <h1 class="font-semibold text-white text-sm tracking-tight">DecentPaste</h1>
                <p class="text-xs text-white/40">${state.deviceInfo?.device_name || 'Loading...'}</p>
              </div>
            </div>
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
            ${this.renderNavItem('history', 'history', 'History')}
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

    return `
      <button
        data-nav="${view}"
        class="nav-item ${isActive ? 'nav-item-active' : ''}"
      >
        ${icon(iconName, 20)}
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
      case 'history':
        return this.renderHistoryView();
      case 'settings':
        return this.renderSettingsView();
      default:
        return this.renderDashboard();
    }
  }

  private renderDashboard(): string {
    const state = store.getState();
    const pairedCount = state.pairedPeers.length;
    const historyCount = state.clipboardHistory.length;
    const recentItems = state.clipboardHistory.slice(0, 3);

    return `
      <div class="flex flex-col h-full">
        <!-- Sticky Top Section -->
        <div class="flex-shrink-0 p-4 pb-0">
          <!-- Stats Grid -->
          <div class="grid grid-cols-2 gap-3 mb-6">
            <div class="card p-4">
              <div class="flex items-center gap-3">
                <div class="icon-container-green">
                  ${icon('users', 18)}
                </div>
                <div>
                  <p class="text-2xl font-bold text-white tracking-tight">${pairedCount}</p>
                  <p class="text-xs text-white/40">Paired Devices</p>
                </div>
              </div>
            </div>
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
            <button id="btn-share-clipboard" class="btn-primary w-full">
              ${icon('share', 18)}
              <span>Share Clipboard</span>
            </button>
          </div>

          <!-- Recent Clipboard Header -->
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Recent Clipboard</h2>
            <button data-nav="history" class="text-xs text-teal-400 hover:text-teal-300 font-medium transition-colors">View all</button>
          </div>
        </div>

        <!-- Scrollable Recent Clipboard Items -->
        <div class="flex-1 min-h-0 overflow-y-auto px-4 pb-4">
          <div id="recent-clipboard" class="space-y-2">
            ${recentItems.length > 0 ? recentItems.map((item) => this.renderClipboardItem(item)).join('') : this.renderEmptyState('No clipboard items yet', 'Copy something to get started')}
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

  private renderHistoryView(): string {
    const history = store.get('clipboardHistory');

    return `
      <div class="p-4 h-full overflow-y-auto">
        <div class="flex items-center justify-between mb-4">
          <div class="flex items-center gap-2">
            <div class="icon-container-teal" style="width: 1.5rem; height: 1.5rem; border-radius: 0.5rem;">
              ${icon('history', 12)}
            </div>
            <h2 class="text-sm font-semibold text-white/80 tracking-tight">Clipboard History</h2>
          </div>
          <button id="btn-clear-all-history" class="btn-danger text-xs px-3 py-1.5">
            ${icon('trash', 12)}
            <span>Clear All</span>
          </button>
        </div>
        <div id="clipboard-history" class="space-y-2">
          ${history.length > 0 ? history.map((item) => this.renderClipboardItem(item)).join('') : this.renderEmptyState('No clipboard history', 'Your copied items will appear here')}
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
              <span class="text-sm text-white/70">Clear history on exit</span>
              <input
                type="checkbox"
                id="clear-on-exit-toggle"
                ${settings.clear_history_on_exit ? 'checked' : ''}
                class="checkbox"
              />
            </label>
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

  private renderClipboardItem(item: ClipboardEntry): string {
    const isLocal = item.is_local;
    // Escape HTML to prevent XSS attacks from malicious clipboard content
    const safeContent = escapeHtml(truncate(item.content, 120));
    return `
      <div class="card p-3 group cursor-pointer" style="transition: all 0.2s ease;">
        <div class="flex items-start justify-between gap-3">
          <div class="flex-1 min-w-0">
            <p class="text-sm text-white/90 break-words line-clamp-2 leading-relaxed">${safeContent}</p>
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
            class="p-2 rounded-lg text-white/30 hover:text-teal-400 hover:bg-teal-500/10 transition-all flex-shrink-0"
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
    if (view === 'history') {
      const container = $('#clipboard-history');
      if (container) {
        const history = store.get('clipboardHistory');
        container.innerHTML =
          history.length > 0
            ? history.map((item) => this.renderClipboardItem(item)).join('')
            : this.renderEmptyState('No clipboard history', 'Your copied items will appear here');
      }
    } else if (view === 'dashboard') {
      const history = store.get('clipboardHistory');

      // Update the clipboard count in stats
      const countEl = $('#clipboard-count');
      if (countEl) {
        countEl.textContent = String(history.length);
      }

      // Update the recent clipboard items
      const container = $('#recent-clipboard');
      if (container) {
        const recentItems = history.slice(0, 3);
        container.innerHTML =
          recentItems.length > 0
            ? recentItems.map((item) => this.renderClipboardItem(item)).join('')
            : this.renderEmptyState('No clipboard items yet', 'Copy something to get started');
      }
    }
  }
}

export async function initApp(rootElement: HTMLElement): Promise<void> {
  const app = new App(rootElement);
  await app.init();
}
