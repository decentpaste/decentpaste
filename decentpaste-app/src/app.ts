import { store, type View, type Toast } from './state/store';
import { eventManager } from './api/events';
import * as commands from './api/commands';
import { readText } from '@tauri-apps/plugin-clipboard-manager';
import { icon } from './components/icons';
import { $, formatTime, truncate, getStatusColor, getStatusText } from './utils/dom';
import type { ClipboardEntry, DiscoveredPeer, PairedPeer } from './api/types';

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

    // Subscribe to state changes
    this.setupStateSubscriptions();
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

    eventManager.on('clipboardReceived', (entry) => {
      store.addClipboardEntry(entry);
      store.addToast(`Clipboard received from ${entry.origin_device_name}`, 'success');
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
        session ? { ...session, pin: payload.pin, peer_name: payload.peerDeviceName, state: 'AwaitingPinConfirmation' } : null
      );
      store.set('pairingModalMode', 'confirm');
    });

    eventManager.on('pairingComplete', (payload) => {
      store.set('showPairingModal', false);
      store.set('activePairingSession', null);
      store.addToast(`Paired with ${payload.deviceName}!`, 'success');
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
    store.subscribe('networkStatus', () => this.updateStatusIndicator());
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
        <div class="flex items-center justify-center h-screen bg-gray-50 dark:bg-gray-900">
          <div class="text-center">
            <div class="inline-block animate-spin mb-4">${icon('loader', 48)}</div>
            <p class="text-gray-600 dark:text-gray-400">Loading DecentPaste...</p>
          </div>
        </div>
      `;
      return;
    }

    this.root.innerHTML = `
      <div class="flex flex-col h-screen bg-gray-50 dark:bg-gray-900">
        <!-- Header -->
        <header class="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 px-4 py-3">
          <div class="flex items-center justify-between">
            <div class="flex items-center gap-3">
              <div class="w-8 h-8 bg-gradient-to-br from-primary-500 to-primary-700 rounded-lg flex items-center justify-center text-white">
                ${icon('clipboard', 18)}
              </div>
              <div>
                <h1 class="font-semibold text-gray-900 dark:text-white">DecentPaste</h1>
                <p class="text-xs text-gray-500 dark:text-gray-400">${state.deviceInfo?.device_name || 'Loading...'}</p>
              </div>
            </div>
            <div id="status-indicator" class="flex items-center gap-2">
              ${this.renderStatusIndicator()}
            </div>
          </div>
        </header>

        <!-- Main Content -->
        <main class="flex-1 overflow-hidden">
          ${this.renderCurrentView()}
        </main>

        <!-- Bottom Navigation -->
        <nav class="bg-white dark:bg-gray-800 border-t border-gray-200 dark:border-gray-700">
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

    this.attachEventListeners();
  }

  private renderStatusIndicator(): string {
    const status = store.get('networkStatus');
    const statusText = getStatusText(status);
    const colorClass = getStatusColor(statusText);

    return `
      <div class="flex items-center gap-2 px-2 py-1 rounded-full bg-gray-100 dark:bg-gray-700">
        <div class="w-2 h-2 rounded-full ${colorClass} ${statusText === 'Connecting' ? 'animate-pulse' : ''}"></div>
        <span class="text-xs text-gray-600 dark:text-gray-300">${statusText}</span>
      </div>
    `;
  }

  private updateStatusIndicator(): void {
    const indicator = $('#status-indicator');
    if (indicator) {
      indicator.innerHTML = this.renderStatusIndicator();
    }
  }

  private renderNavItem(view: View, iconName: keyof typeof import('./components/icons').icons, label: string): string {
    const currentView = store.get('currentView');
    const isActive = currentView === view;
    const activeClass = isActive ? 'text-primary-600 dark:text-primary-400' : 'text-gray-500 dark:text-gray-400';

    return `
      <button
        data-nav="${view}"
        class="flex flex-col items-center gap-1 px-4 py-1 ${activeClass} hover:text-primary-600 dark:hover:text-primary-400 transition-colors"
      >
        ${icon(iconName, 20)}
        <span class="text-xs">${label}</span>
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
      <div class="p-4 h-full overflow-y-auto">
        <!-- Stats -->
        <div class="grid grid-cols-2 gap-3 mb-6">
          <div class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-green-100 dark:bg-green-900/30 rounded-lg flex items-center justify-center text-green-600 dark:text-green-400">
                ${icon('users', 20)}
              </div>
              <div>
                <p class="text-2xl font-bold text-gray-900 dark:text-white">${pairedCount}</p>
                <p class="text-xs text-gray-500 dark:text-gray-400">Paired Devices</p>
              </div>
            </div>
          </div>
          <div class="bg-white dark:bg-gray-800 rounded-xl p-4 border border-gray-200 dark:border-gray-700">
            <div class="flex items-center gap-3">
              <div class="w-10 h-10 bg-blue-100 dark:bg-blue-900/30 rounded-lg flex items-center justify-center text-blue-600 dark:text-blue-400">
                ${icon('clipboard', 20)}
              </div>
              <div>
                <p class="text-2xl font-bold text-gray-900 dark:text-white">${historyCount}</p>
                <p class="text-xs text-gray-500 dark:text-gray-400">Clipboard Items</p>
              </div>
            </div>
          </div>
        </div>

        <!-- Quick Actions -->
        <div class="mb-6">
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Quick Actions</h2>
          <div class="flex gap-3 mb-3">
            <button
              id="btn-share-clipboard"
              class="flex-1 bg-primary-600 hover:bg-primary-700 text-white rounded-xl p-3 flex items-center justify-center gap-2 transition-colors"
            >
              ${icon('share', 18)}
              <span class="text-sm font-medium">Share Clipboard</span>
            </button>
          </div>
          <div class="flex gap-3">
            <button
              id="btn-refresh-peers"
              class="flex-1 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-3 flex items-center justify-center gap-2 hover:bg-gray-50 dark:hover:bg-gray-750 transition-colors"
            >
              ${icon('refreshCw', 18, 'text-gray-600 dark:text-gray-400')}
              <span class="text-sm text-gray-700 dark:text-gray-300">Refresh</span>
            </button>
            <button
              id="btn-clear-history"
              class="flex-1 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-xl p-3 flex items-center justify-center gap-2 hover:bg-gray-50 dark:hover:bg-gray-750 transition-colors"
            >
              ${icon('trash', 18, 'text-gray-600 dark:text-gray-400')}
              <span class="text-sm text-gray-700 dark:text-gray-300">Clear History</span>
            </button>
          </div>
        </div>

        <!-- Recent Clipboard -->
        <div>
          <div class="flex items-center justify-between mb-3">
            <h2 class="text-sm font-semibold text-gray-900 dark:text-white">Recent Clipboard</h2>
            <button data-nav="history" class="text-xs text-primary-600 dark:text-primary-400 hover:underline">View all</button>
          </div>
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
    const discoveredPeers = state.discoveredPeers.filter(
      (d) => !pairedPeers.some((p) => p.peer_id === d.peer_id)
    );

    return `
      <div class="p-4 h-full overflow-y-auto">
        <!-- Paired Devices -->
        <div class="mb-6">
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Paired Devices</h2>
          <div id="paired-peers" class="space-y-2">
            ${pairedPeers.length > 0 ? pairedPeers.map((peer) => this.renderPairedPeer(peer)).join('') : this.renderEmptyState('No paired devices', 'Pair with a device below')}
          </div>
        </div>

        <!-- Discovered Devices -->
        <div>
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Discovered Devices</h2>
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
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white">Clipboard History</h2>
          <button
            id="btn-clear-all-history"
            class="text-xs text-red-600 dark:text-red-400 hover:underline"
          >
            Clear All
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
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Device</h2>
          <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
            <div class="flex items-center gap-3 mb-3">
              <div class="w-12 h-12 bg-primary-100 dark:bg-primary-900/30 rounded-xl flex items-center justify-center text-primary-600 dark:text-primary-400">
                ${icon('monitor', 24)}
              </div>
              <div>
                <input
                  id="device-name-input"
                  type="text"
                  value="${settings.device_name}"
                  class="font-semibold text-gray-900 dark:text-white bg-transparent border-none p-0 focus:outline-none focus:ring-0"
                />
                <p class="text-xs text-gray-500 dark:text-gray-400">${deviceInfo?.device_id?.slice(0, 8) || 'Unknown'}...</p>
              </div>
            </div>
          </div>
        </div>

        <!-- Sync Settings -->
        <div class="mb-6">
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">Sync</h2>
          <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 divide-y divide-gray-200 dark:divide-gray-700">
            <label class="flex items-center justify-between p-4 cursor-pointer">
              <span class="text-sm text-gray-700 dark:text-gray-300">Auto-sync clipboard</span>
              <input
                type="checkbox"
                id="auto-sync-toggle"
                ${settings.auto_sync_enabled ? 'checked' : ''}
                class="w-5 h-5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
            </label>
            <label class="flex items-center justify-between p-4 cursor-pointer">
              <span class="text-sm text-gray-700 dark:text-gray-300">Show notifications</span>
              <input
                type="checkbox"
                id="notifications-toggle"
                ${settings.show_notifications ? 'checked' : ''}
                class="w-5 h-5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
            </label>
          </div>
        </div>

        <!-- History Settings -->
        <div class="mb-6">
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">History</h2>
          <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 divide-y divide-gray-200 dark:divide-gray-700">
            <div class="flex items-center justify-between p-4">
              <span class="text-sm text-gray-700 dark:text-gray-300">History limit</span>
              <select
                id="history-limit-select"
                class="bg-gray-100 dark:bg-gray-700 border-none rounded-lg text-sm text-gray-900 dark:text-white px-3 py-1"
              >
                <option value="25" ${settings.clipboard_history_limit === 25 ? 'selected' : ''}>25 items</option>
                <option value="50" ${settings.clipboard_history_limit === 50 ? 'selected' : ''}>50 items</option>
                <option value="100" ${settings.clipboard_history_limit === 100 ? 'selected' : ''}>100 items</option>
              </select>
            </div>
            <label class="flex items-center justify-between p-4 cursor-pointer">
              <span class="text-sm text-gray-700 dark:text-gray-300">Clear history on exit</span>
              <input
                type="checkbox"
                id="clear-on-exit-toggle"
                ${settings.clear_history_on_exit ? 'checked' : ''}
                class="w-5 h-5 rounded border-gray-300 text-primary-600 focus:ring-primary-500"
              />
            </label>
          </div>
        </div>

        <!-- About -->
        <div>
          <h2 class="text-sm font-semibold text-gray-900 dark:text-white mb-3">About</h2>
          <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-4">
            <p class="text-sm text-gray-600 dark:text-gray-400">DecentPaste v0.1.0</p>
            <p class="text-xs text-gray-500 dark:text-gray-500 mt-1">Cross-platform clipboard sharing over P2P</p>
          </div>
        </div>
      </div>
    `;
  }

  private renderClipboardItem(item: ClipboardEntry): string {
    return `
      <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-3 group hover:border-primary-300 dark:hover:border-primary-600 transition-colors">
        <div class="flex items-start justify-between gap-2">
          <div class="flex-1 min-w-0">
            <p class="text-sm text-gray-900 dark:text-white break-words line-clamp-2">${truncate(item.content, 100)}</p>
            <div class="flex items-center gap-2 mt-2">
              <span class="text-xs text-gray-500 dark:text-gray-400">
                ${item.is_local ? 'Local' : item.origin_device_name}
              </span>
              <span class="text-xs text-gray-400 dark:text-gray-500">&bull;</span>
              <span class="text-xs text-gray-400 dark:text-gray-500">${formatTime(item.timestamp)}</span>
            </div>
          </div>
          <button
            data-copy="${item.id}"
            class="p-2 text-gray-400 hover:text-primary-600 dark:hover:text-primary-400 opacity-0 group-hover:opacity-100 transition-opacity"
            title="Copy to clipboard"
          >
            ${icon('copy', 16)}
          </button>
        </div>
      </div>
    `;
  }

  private renderPairedPeer(peer: PairedPeer): string {
    return `
      <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-3 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 bg-green-100 dark:bg-green-900/30 rounded-lg flex items-center justify-center">
            ${icon('monitor', 20, 'text-green-600 dark:text-green-400')}
          </div>
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">${peer.device_name}</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">
              ${peer.last_seen ? `Last seen ${formatTime(peer.last_seen)}` : 'Paired'}
            </p>
          </div>
        </div>
        <button
          data-unpair="${peer.peer_id}"
          class="p-2 text-gray-400 hover:text-red-500 transition-colors"
          title="Unpair device"
        >
          ${icon('unlink', 18)}
        </button>
      </div>
    `;
  }

  private renderDiscoveredPeer(peer: DiscoveredPeer): string {
    return `
      <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-3 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <div class="w-10 h-10 bg-yellow-100 dark:bg-yellow-900/30 rounded-lg flex items-center justify-center">
            ${icon('smartphone', 20, 'text-yellow-600 dark:text-yellow-400')}
          </div>
          <div>
            <p class="text-sm font-medium text-gray-900 dark:text-white">${peer.device_name || 'Unknown Device'}</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">Discovered ${formatTime(peer.discovered_at)}</p>
          </div>
        </div>
        <button
          data-pair="${peer.peer_id}"
          class="px-3 py-1.5 bg-primary-600 hover:bg-primary-700 text-white text-sm font-medium rounded-lg transition-colors"
        >
          Pair
        </button>
      </div>
    `;
  }

  private renderEmptyState(title: string, subtitle: string): string {
    return `
      <div class="text-center py-8">
        <p class="text-gray-600 dark:text-gray-400 text-sm">${title}</p>
        <p class="text-gray-400 dark:text-gray-500 text-xs mt-1">${subtitle}</p>
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
    const bgColor =
      toast.type === 'success'
        ? 'bg-green-500'
        : toast.type === 'error'
        ? 'bg-red-500'
        : 'bg-gray-700';

    return `
      <div class="${bgColor} text-white px-4 py-3 rounded-lg shadow-lg flex items-center justify-between animate-slide-up">
        <span class="text-sm">${toast.message}</span>
        <button data-dismiss-toast="${toast.id}" class="ml-3 hover:opacity-80">
          ${icon('x', 16)}
        </button>
      </div>
    `;
  }

  private renderPairingModalContent(): string {
    const session = store.get('activePairingSession');
    const mode = store.get('pairingModalMode');

    if (!session) return '';

    let content = '';

    if (mode === 'respond') {
      content = `
        <div class="text-center">
          <div class="w-16 h-16 bg-primary-100 dark:bg-primary-900/30 rounded-full flex items-center justify-center mx-auto mb-4">
            ${icon('link', 32, 'text-primary-600 dark:text-primary-400')}
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Pairing Request</h3>
          <p class="text-gray-600 dark:text-gray-400 mb-6">${session.peer_name || 'A device'} wants to pair with you</p>
          <div class="flex gap-3">
            <button
              id="btn-reject-pairing"
              class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
              style="touch-action: manipulation"
            >
              Reject
            </button>
            <button
              id="btn-accept-pairing"
              class="flex-1 px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg transition-colors"
              style="touch-action: manipulation"
            >
              Accept
            </button>
          </div>
        </div>
      `;
    } else if (mode === 'confirm' && session.pin) {
      // Initiator shows Confirm button, Responder shows waiting message
      const isInitiator = session.is_initiator;
      const buttonArea = isInitiator ? `
          <div class="flex gap-3">
            <button
              id="btn-cancel-pairing"
              class="flex-1 px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
              style="touch-action: manipulation"
            >
              Cancel
            </button>
            <button
              id="btn-confirm-pin"
              class="flex-1 px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded-lg transition-colors"
              style="touch-action: manipulation"
            >
              Confirm
            </button>
          </div>
      ` : `
          <p class="text-gray-500 dark:text-gray-400 text-sm mb-4">Waiting for other device to confirm...</p>
          <button
            id="btn-cancel-pairing"
            class="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
            style="touch-action: manipulation"
          >
            Cancel
          </button>
      `;

      content = `
        <div class="text-center">
          <div class="w-16 h-16 bg-primary-100 dark:bg-primary-900/30 rounded-full flex items-center justify-center mx-auto mb-4">
            ${icon('check', 32, 'text-primary-600 dark:text-primary-400')}
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Confirm PIN</h3>
          <p class="text-gray-600 dark:text-gray-400 mb-4">Verify this PIN matches on both devices</p>
          <div class="text-4xl font-mono font-bold text-primary-600 dark:text-primary-400 tracking-widest mb-6">
            ${session.pin}
          </div>
          ${buttonArea}
        </div>
      `;
    } else if (mode === 'initiate') {
      content = `
        <div class="text-center">
          <div class="animate-spin w-16 h-16 mx-auto mb-4">
            ${icon('loader', 64, 'text-primary-600 dark:text-primary-400')}
          </div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-2">Pairing...</h3>
          <p class="text-gray-600 dark:text-gray-400 mb-6">Waiting for ${session.peer_name || 'device'} to respond</p>
          <button
            id="btn-cancel-pairing"
            class="px-4 py-2 border border-gray-300 dark:border-gray-600 rounded-lg text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700 transition-colors"
          >
            Cancel
          </button>
        </div>
      `;
    }

    return `
      <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
        <div class="bg-white dark:bg-gray-800 rounded-2xl p-6 m-4 max-w-sm w-full shadow-xl">
          ${content}
        </div>
      </div>
    `;
  }

  private renderPairingModal(): void {
    // Debounce rapid re-renders (e.g., when multiple state changes happen at once)
    if (this.modalRenderPending) return;
    this.modalRenderPending = true;

    // Use microtask to batch multiple state changes
    queueMicrotask(() => {
      this.modalRenderPending = false;
      const modal = $('#pairing-modal');
      if (modal) {
        const show = store.get('showPairingModal');
        modal.className = show ? '' : 'hidden';
        modal.innerHTML = this.renderPairingModalContent();
        this.attachPairingModalListeners();
      }
    });
  }

  private renderPeersList(): void {
    const view = store.get('currentView');
    if (view === 'peers') {
      const content = $('main');
      if (content) {
        content.innerHTML = this.renderPeersView();
        this.attachEventListeners();
      }
    }
  }

  private renderClipboardHistory(): void {
    const view = store.get('currentView');
    if (view === 'history') {
      const container = $('#clipboard-history');
      if (container) {
        const history = store.get('clipboardHistory');
        container.innerHTML = history.length > 0
          ? history.map((item) => this.renderClipboardItem(item)).join('')
          : this.renderEmptyState('No clipboard history', 'Your copied items will appear here');
        this.attachEventListeners();
      }
    } else if (view === 'dashboard') {
      const container = $('#recent-clipboard');
      if (container) {
        const recentItems = store.get('clipboardHistory').slice(0, 3);
        container.innerHTML = recentItems.length > 0
          ? recentItems.map((item) => this.renderClipboardItem(item)).join('')
          : this.renderEmptyState('No clipboard items yet', 'Copy something to get started');
        this.attachEventListeners();
      }
    }
  }

  private attachEventListeners(): void {
    // Navigation
    document.querySelectorAll('[data-nav]').forEach((el) => {
      el.addEventListener('click', () => {
        const view = el.getAttribute('data-nav') as View;
        store.set('currentView', view);
      });
    });

    // Copy buttons
    document.querySelectorAll('[data-copy]').forEach((el) => {
      el.addEventListener('click', async () => {
        const id = el.getAttribute('data-copy');
        const history = store.get('clipboardHistory');
        const item = history.find((h) => h.id === id);
        if (item) {
          await commands.setClipboard(item.content);
          store.addToast('Copied to clipboard', 'success');
        }
      });
    });

    // Pair buttons
    document.querySelectorAll('[data-pair]').forEach((el) => {
      el.addEventListener('click', async () => {
        const peerId = el.getAttribute('data-pair');
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
            store.addToast(`Failed to initiate pairing: ${error}`, 'error');
            store.set('showPairingModal', false);
            store.set('activePairingSession', null);
          }
        }
      });
    });

    // Unpair buttons
    document.querySelectorAll('[data-unpair]').forEach((el) => {
      el.addEventListener('click', async () => {
        const peerId = el.getAttribute('data-unpair');
        if (peerId) {
          try {
            await commands.removePairedPeer(peerId);
            store.removePairedPeer(peerId);
            store.addToast('Device unpaired', 'success');
          } catch (error) {
            store.addToast(`Failed to unpair: ${error}`, 'error');
          }
        }
      });
    });

    // Dismiss toasts
    document.querySelectorAll('[data-dismiss-toast]').forEach((el) => {
      el.addEventListener('click', () => {
        const id = el.getAttribute('data-dismiss-toast');
        if (id) store.removeToast(id);
      });
    });

    // Refresh peers
    $('#btn-refresh-peers')?.addEventListener('click', async () => {
      const peers = await commands.getDiscoveredPeers();
      store.set('discoveredPeers', peers);
      store.addToast('Refreshed peer list', 'info');
    });

    // Clear history
    $('#btn-clear-history')?.addEventListener('click', async () => {
      await commands.clearClipboardHistory();
      store.set('clipboardHistory', []);
      store.addToast('History cleared', 'success');
    });

    // Share clipboard (reads clipboard and sends to peers)
    $('#btn-share-clipboard')?.addEventListener('click', async () => {
      try {
        const content = await readText();
        if (!content || content.trim() === '') {
          store.addToast('Clipboard is empty', 'error');
          return;
        }
        await commands.shareClipboardContent(content);
        store.addToast('Clipboard shared with peers', 'success');
      } catch (e) {
        const error = e instanceof Error ? e.message : String(e);
        store.addToast(`Failed to share: ${error}`, 'error');
      }
    });

    $('#btn-clear-all-history')?.addEventListener('click', async () => {
      await commands.clearClipboardHistory();
      store.set('clipboardHistory', []);
      store.addToast('History cleared', 'success');
    });

    // Settings listeners
    this.attachSettingsListeners();
    this.attachPairingModalListeners();
  }

  private attachSettingsListeners(): void {
    // Auto-sync toggle
    $('#auto-sync-toggle')?.addEventListener('change', async (e) => {
      const checked = (e.target as HTMLInputElement).checked;
      const settings = { ...store.get('settings'), auto_sync_enabled: checked };
      await commands.updateSettings(settings);
      store.set('settings', settings);
    });

    // Notifications toggle
    $('#notifications-toggle')?.addEventListener('change', async (e) => {
      const checked = (e.target as HTMLInputElement).checked;
      const settings = { ...store.get('settings'), show_notifications: checked };
      await commands.updateSettings(settings);
      store.set('settings', settings);
    });

    // History limit select
    $('#history-limit-select')?.addEventListener('change', async (e) => {
      const value = parseInt((e.target as HTMLSelectElement).value, 10);
      const settings = { ...store.get('settings'), clipboard_history_limit: value };
      await commands.updateSettings(settings);
      store.set('settings', settings);
    });

    // Clear on exit toggle
    $('#clear-on-exit-toggle')?.addEventListener('change', async (e) => {
      const checked = (e.target as HTMLInputElement).checked;
      const settings = { ...store.get('settings'), clear_history_on_exit: checked };
      await commands.updateSettings(settings);
      store.set('settings', settings);
    });

    // Device name input
    $('#device-name-input')?.addEventListener('blur', async (e) => {
      const value = (e.target as HTMLInputElement).value.trim();
      if (value) {
        const settings = { ...store.get('settings'), device_name: value };
        await commands.updateSettings(settings);
        store.set('settings', settings);
      }
    });
  }

  private attachPairingModalListeners(): void {
    // Accept pairing
    const acceptBtn = $('#btn-accept-pairing') as HTMLButtonElement | null;
    acceptBtn?.addEventListener('click', async () => {
      const session = store.get('activePairingSession');
      // Guard against duplicate calls using both button state AND app flag
      if (session && !acceptBtn.disabled && !this.pairingInProgress) {
        acceptBtn.disabled = true;
        this.pairingInProgress = true;
        acceptBtn.textContent = 'Accepting...';
        try {
          const pin = await commands.respondToPairing(session.session_id, true);
          if (pin) {
            store.update('activePairingSession', (s) =>
              s ? { ...s, pin, state: 'AwaitingPinConfirmation' } : null
            );
            store.set('pairingModalMode', 'confirm');
          }
        } catch (error) {
          store.addToast(`Failed to accept pairing: ${error}`, 'error');
          acceptBtn.disabled = false;
          acceptBtn.textContent = 'Accept';
        } finally {
          this.pairingInProgress = false;
        }
      }
    });

    // Reject pairing
    const rejectBtn = $('#btn-reject-pairing') as HTMLButtonElement | null;
    rejectBtn?.addEventListener('click', async () => {
      const session = store.get('activePairingSession');
      if (session && !rejectBtn.disabled) {
        rejectBtn.disabled = true;
        try {
          await commands.respondToPairing(session.session_id, false);
        } catch (error) {
          store.addToast(`Failed to reject pairing: ${error}`, 'error');
        } finally {
          store.set('showPairingModal', false);
          store.set('activePairingSession', null);
        }
      }
    });

    // Confirm PIN
    const confirmBtn = $('#btn-confirm-pin') as HTMLButtonElement | null;
    confirmBtn?.addEventListener('click', async () => {
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
          // If success, the pairing-complete event will close the modal
        } catch (error) {
          store.addToast(`Failed to confirm pairing: ${error}`, 'error');
          confirmBtn.disabled = false;
          confirmBtn.textContent = 'Confirm';
        }
      }
    });

    // Cancel pairing
    const cancelBtn = $('#btn-cancel-pairing') as HTMLButtonElement | null;
    cancelBtn?.addEventListener('click', async () => {
      const session = store.get('activePairingSession');
      if (session && !cancelBtn.disabled) {
        cancelBtn.disabled = true;
        await commands.cancelPairing(session.session_id);
      }
      store.set('showPairingModal', false);
      store.set('activePairingSession', null);
    });
  }
}

export async function initApp(rootElement: HTMLElement): Promise<void> {
  const app = new App(rootElement);
  await app.init();
}
