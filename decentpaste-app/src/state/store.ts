import type {
  AppSettings,
  ClipboardEntry,
  DeviceInfo,
  DiscoveredPeer,
  NetworkStatus,
  PairedPeer,
  PairingSession,
} from '../api/types';

export type View = 'dashboard' | 'peers' | 'history' | 'settings';

export interface Toast {
  id: string;
  message: string;
  type: 'success' | 'error' | 'info';
  duration?: number;
}

export interface AppState {
  // Network
  networkStatus: NetworkStatus;

  // Peers
  discoveredPeers: DiscoveredPeer[];
  pairedPeers: PairedPeer[];

  // Clipboard
  clipboardHistory: ClipboardEntry[];

  // Pairing
  activePairingSession: PairingSession | null;

  // UI
  currentView: View;
  toasts: Toast[];
  showPairingModal: boolean;
  pairingModalMode: 'initiate' | 'respond' | 'confirm' | null;

  // Settings & Device
  settings: AppSettings;
  deviceInfo: DeviceInfo | null;

  // Loading states
  isLoading: boolean;
}

type StateListener<K extends keyof AppState> = (value: AppState[K]) => void;

class Store {
  private state: AppState;
  private listeners: Map<keyof AppState, Set<StateListener<any>>> = new Map();
  private toastTimers: Map<string, ReturnType<typeof setTimeout>> = new Map();

  constructor() {
    this.state = {
      networkStatus: 'Disconnected',
      discoveredPeers: [],
      pairedPeers: [],
      clipboardHistory: [],
      activePairingSession: null,
      currentView: 'dashboard',
      toasts: [],
      showPairingModal: false,
      pairingModalMode: null,
      settings: {
        device_name: 'My Device',
        auto_sync_enabled: true,
        clipboard_history_limit: 50,
        clear_history_on_exit: false,
        show_notifications: true,
        clipboard_poll_interval_ms: 500,
      },
      deviceInfo: null,
      isLoading: true,
    };
  }

  getState(): AppState {
    return { ...this.state };
  }

  get<K extends keyof AppState>(key: K): AppState[K] {
    return this.state[key];
  }

  set<K extends keyof AppState>(key: K, value: AppState[K]): void {
    this.state[key] = value;
    this.notify(key);
  }

  update<K extends keyof AppState>(key: K, updater: (value: AppState[K]) => AppState[K]): void {
    this.state[key] = updater(this.state[key]);
    this.notify(key);
  }

  subscribe<K extends keyof AppState>(key: K, listener: StateListener<K>): () => void {
    if (!this.listeners.has(key)) {
      this.listeners.set(key, new Set());
    }
    this.listeners.get(key)!.add(listener);

    // Return unsubscribe function
    return () => {
      this.listeners.get(key)?.delete(listener);
    };
  }

  private notify<K extends keyof AppState>(key: K): void {
    const listeners = this.listeners.get(key);
    if (listeners) {
      listeners.forEach((listener) => listener(this.state[key]));
    }
  }

  // Helper methods
  addToast(message: string, type: Toast['type'] = 'info', duration = 3000): void {
    const id = Math.random().toString(36).slice(2);
    const toast: Toast = { id, message, type, duration };

    this.update('toasts', (toasts) => [...toasts, toast]);

    if (duration > 0) {
      const timerId = setTimeout(() => {
        this.removeToast(id);
      }, duration);
      this.toastTimers.set(id, timerId);
    }
  }

  removeToast(id: string): void {
    // Clear any pending timer for this toast
    const timerId = this.toastTimers.get(id);
    if (timerId) {
      clearTimeout(timerId);
      this.toastTimers.delete(id);
    }
    this.update('toasts', (toasts) => toasts.filter((t) => t.id !== id));
  }

  /**
   * Clears all toasts and their associated timers.
   * Useful for cleanup when the app unmounts.
   */
  clearAllToasts(): void {
    // Clear all pending timers
    for (const timerId of this.toastTimers.values()) {
      clearTimeout(timerId);
    }
    this.toastTimers.clear();
    this.set('toasts', []);
  }

  addClipboardEntry(entry: ClipboardEntry): void {
    this.update('clipboardHistory', (history) => {
      // Check for duplicates
      if (history.some((e) => e.content_hash === entry.content_hash)) {
        return history;
      }
      // Add to front, limit to settings limit
      const limit = this.state.settings.clipboard_history_limit;
      return [entry, ...history].slice(0, limit);
    });
  }

  addDiscoveredPeer(peer: DiscoveredPeer): void {
    this.update('discoveredPeers', (peers) => {
      const existing = peers.findIndex((p) => p.peer_id === peer.peer_id);
      if (existing >= 0) {
        const updated = [...peers];
        updated[existing] = peer;
        return updated;
      }
      return [...peers, peer];
    });
  }

  removeDiscoveredPeer(peerId: string): void {
    this.update('discoveredPeers', (peers) => peers.filter((p) => p.peer_id !== peerId));
  }

  addPairedPeer(peer: PairedPeer): void {
    this.update('pairedPeers', (peers) => {
      if (peers.some((p) => p.peer_id === peer.peer_id)) {
        return peers;
      }
      return [...peers, peer];
    });
  }

  removePairedPeer(peerId: string): void {
    this.update('pairedPeers', (peers) => peers.filter((p) => p.peer_id !== peerId));
  }

  updatePeerName(peerId: string, deviceName: string): void {
    // Update in discovered peers
    this.update('discoveredPeers', (peers) =>
      peers.map((p) => (p.peer_id === peerId ? { ...p, device_name: deviceName } : p)),
    );

    // Update in paired peers
    this.update('pairedPeers', (peers) =>
      peers.map((p) => (p.peer_id === peerId ? { ...p, device_name: deviceName } : p)),
    );
  }
}

export const store = new Store();
