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
} from './types';

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
  networkError: EventHandler<string>[];
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
    networkError: [],
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
      listen<string>('network-error', (e) => {
        this.listeners.networkError.forEach((fn) => fn(e.payload));
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
