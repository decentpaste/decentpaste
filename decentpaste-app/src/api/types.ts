// Network types
export type NetworkStatus = 'Disconnected' | 'Connecting' | 'Connected' | { Error: string };

export interface DiscoveredPeer {
    peer_id: string;
    device_name: string | null;
    addresses: string[];
    discovered_at: string;
    is_paired: boolean;
}

export interface PairedPeer {
    peer_id: string;
    device_name: string;
    paired_at: string;
    last_seen: string | null;
}

export interface ConnectedPeer {
    peer_id: string;
    device_name: string;
    connected_at: string;
}

// Clipboard types
export interface ClipboardEntry {
    id: string;
    content: string;
    content_hash: string;
    timestamp: string;
    origin_device_id: string;
    origin_device_name: string;
    is_local: boolean;
}

// Pairing types
export type PairingState =
    | 'Initiated'
    | 'AwaitingPinConfirmation'
    | 'AwaitingPeerConfirmation'
    | 'Completed'
    | { Failed: string };

export interface PairingSession {
    session_id: string;
    peer_id: string;
    peer_name: string | null;
    pin: string | null;
    state: PairingState;
    is_initiator: boolean;
    created_at: string;
}

// Settings types
export interface AppSettings {
    device_name: string;
    auto_sync_enabled: boolean;
    clipboard_history_limit: number;
    clear_history_on_exit: boolean;
    show_notifications: boolean;
    clipboard_poll_interval_ms: number;
}

// Device info
export interface DeviceInfo {
    device_id: string;
    device_name: string;
    peer_id: string | null;
}

// Event payloads
export interface PairingRequestPayload {
    sessionId: string;
    peerId: string;
    deviceName: string;
}

export interface PairingPinPayload {
    sessionId: string;
    pin: string;
    peerDeviceName: string;
}

export interface PairingCompletePayload {
    sessionId: string;
    peerId: string;
    deviceName: string;
}

export interface PairingFailedPayload {
    sessionId: string;
    error: string;
}

export interface ClipboardBroadcastPayload {
    id: string;
    peerCount: number;
}
