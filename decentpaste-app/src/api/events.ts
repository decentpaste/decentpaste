import { listen, UnlistenFn } from '@tauri-apps/api/event';
import type {
  ClipboardBroadcastPayload,
  ClipboardEntry,
  DiscoveredPeer,
  NetworkStatus,
  PairingCompletePayload,
  PairingFailedPayload,
  PairingPinPayload,
  PairingRequestPayload,
  PeerNameUpdatedPayload,
  VaultStatus,
} from './types';

/** Payload for clipboard synced while app was in background (Android) */
export interface ClipboardSyncedFromBackgroundPayload {
  content: string;
  fromDevice: string;
}

/** Payload for settings changed from system tray */
export interface SettingsChangedPayload {
  auto_sync_enabled?: boolean;
}

// ============================================================================
// Internet Connectivity Events
// ============================================================================

/** Connection type for a peer */
export type ConnectionType = 'local' | 'direct' | 'relay';

/** Payload for connection type changed event */
export interface ConnectionTypeChangedPayload {
  peerId: string;
  connectionType: ConnectionType;
}

/** Payload for relay connected event */
export interface RelayConnectedPayload {
  relayPeerId: string;
  relayAddress: string;
}

/** Payload for relay disconnected event */
export interface RelayDisconnectedPayload {
  relayPeerId: string;
}

/** Payload for NAT status detected event */
export interface NatStatusPayload {
  isPublic: boolean;
}

/** Payload for hole punch result event */
export interface HolePunchResultPayload {
  peerId: string;
  success: boolean;
}

export type EventHandler<T> = (payload: T) => void;

interface EventListeners {
  networkStatus: EventHandler<NetworkStatus>[];
  peerDiscovered: EventHandler<DiscoveredPeer>[];
  peerLost: EventHandler<string>[];
  peerNameUpdated: EventHandler<PeerNameUpdatedPayload>[];
  pairingRequest: EventHandler<PairingRequestPayload>[];
  pairingPin: EventHandler<PairingPinPayload>[];
  pairingComplete: EventHandler<PairingCompletePayload>[];
  pairingFailed: EventHandler<PairingFailedPayload>[];
  clipboardReceived: EventHandler<ClipboardEntry>[];
  clipboardSent: EventHandler<ClipboardEntry>[];
  clipboardBroadcast: EventHandler<ClipboardBroadcastPayload>[];
  clipboardSyncedFromBackground: EventHandler<ClipboardSyncedFromBackgroundPayload>[];
  networkError: EventHandler<string>[];
  appMinimizedToTray: EventHandler<void>[];
  vaultStatus: EventHandler<VaultStatus>[];
  settingsChanged: EventHandler<SettingsChangedPayload>[];
  // Internet connectivity events
  connectionTypeChanged: EventHandler<ConnectionTypeChangedPayload>[];
  relayConnected: EventHandler<RelayConnectedPayload>[];
  relayDisconnected: EventHandler<RelayDisconnectedPayload>[];
  natStatus: EventHandler<NatStatusPayload>[];
  holePunchResult: EventHandler<HolePunchResultPayload>[];
}

class EventManager {
  private listeners: EventListeners = {
    networkStatus: [],
    peerDiscovered: [],
    peerLost: [],
    peerNameUpdated: [],
    pairingRequest: [],
    pairingPin: [],
    pairingComplete: [],
    pairingFailed: [],
    clipboardReceived: [],
    clipboardSent: [],
    clipboardBroadcast: [],
    clipboardSyncedFromBackground: [],
    networkError: [],
    appMinimizedToTray: [],
    vaultStatus: [],
    settingsChanged: [],
    // Internet connectivity events
    connectionTypeChanged: [],
    relayConnected: [],
    relayDisconnected: [],
    natStatus: [],
    holePunchResult: [],
  };

  private unlistenFns: UnlistenFn[] = [];

  async setup(): Promise<void> {
    this.unlistenFns = await Promise.all([
      listen<NetworkStatus>('network-status', (e) => {
        this.listeners.networkStatus.forEach((fn) => fn(e.payload));
      }),
      listen<DiscoveredPeer>('peer-discovered', (e) => {
        this.listeners.peerDiscovered.forEach((fn) => fn(e.payload));
      }),
      listen<string>('peer-lost', (e) => {
        this.listeners.peerLost.forEach((fn) => fn(e.payload));
      }),
      listen<PeerNameUpdatedPayload>('peer-name-updated', (e) => {
        this.listeners.peerNameUpdated.forEach((fn) => fn(e.payload));
      }),
      listen<PairingRequestPayload>('pairing-request', (e) => {
        this.listeners.pairingRequest.forEach((fn) => fn(e.payload));
      }),
      listen<PairingPinPayload>('pairing-pin', (e) => {
        this.listeners.pairingPin.forEach((fn) => fn(e.payload));
      }),
      listen<PairingCompletePayload>('pairing-complete', (e) => {
        this.listeners.pairingComplete.forEach((fn) => fn(e.payload));
      }),
      listen<PairingFailedPayload>('pairing-failed', (e) => {
        this.listeners.pairingFailed.forEach((fn) => fn(e.payload));
      }),
      listen<ClipboardEntry>('clipboard-received', (e) => {
        this.listeners.clipboardReceived.forEach((fn) => fn(e.payload));
      }),
      listen<ClipboardEntry>('clipboard-sent', (e) => {
        this.listeners.clipboardSent.forEach((fn) => fn(e.payload));
      }),
      listen<ClipboardBroadcastPayload>('clipboard-broadcast', (e) => {
        this.listeners.clipboardBroadcast.forEach((fn) => fn(e.payload));
      }),
      listen<ClipboardSyncedFromBackgroundPayload>('clipboard-synced-from-background', (e) => {
        this.listeners.clipboardSyncedFromBackground.forEach((fn) => fn(e.payload));
      }),
      listen<string>('network-error', (e) => {
        this.listeners.networkError.forEach((fn) => fn(e.payload));
      }),
      listen('app-minimized-to-tray', () => {
        this.listeners.appMinimizedToTray.forEach((fn) => fn());
      }),
      listen<VaultStatus>('vault-status', (e) => {
        this.listeners.vaultStatus.forEach((fn) => fn(e.payload));
      }),
      listen<SettingsChangedPayload>('settings-changed', (e) => {
        this.listeners.settingsChanged.forEach((fn) => fn(e.payload));
      }),
      // Internet connectivity events
      listen<ConnectionTypeChangedPayload>('connection-type-changed', (e) => {
        this.listeners.connectionTypeChanged.forEach((fn) => fn(e.payload));
      }),
      listen<RelayConnectedPayload>('relay-connected', (e) => {
        this.listeners.relayConnected.forEach((fn) => fn(e.payload));
      }),
      listen<RelayDisconnectedPayload>('relay-disconnected', (e) => {
        this.listeners.relayDisconnected.forEach((fn) => fn(e.payload));
      }),
      listen<NatStatusPayload>('nat-status', (e) => {
        this.listeners.natStatus.forEach((fn) => fn(e.payload));
      }),
      listen<HolePunchResultPayload>('hole-punch-result', (e) => {
        this.listeners.holePunchResult.forEach((fn) => fn(e.payload));
      }),
    ]);
  }

  cleanup(): void {
    this.unlistenFns.forEach((fn) => fn());
    this.unlistenFns = [];
  }

  on<K extends keyof EventListeners>(event: K, handler: EventListeners[K][number]): () => void {
    (this.listeners[event] as any[]).push(handler);
    return () => {
      const index = (this.listeners[event] as any[]).indexOf(handler);
      if (index > -1) {
        (this.listeners[event] as any[]).splice(index, 1);
      }
    };
  }
}

export const eventManager = new EventManager();
