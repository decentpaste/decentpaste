use chrono::Utc;
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, mdns, noise,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Maximum number of connection retries per peer
const MAX_CONNECTION_RETRIES: u32 = 3;
/// Delay between connection retries
const RETRY_DELAY: Duration = Duration::from_secs(2);

use super::behaviour::{
    DecentPasteBehaviour, PairingRequest as ReqPairingRequest,
    PairingResponse as ReqPairingResponse,
};
use super::events::{ConnectedPeer, DiscoveredPeer, NetworkEvent, NetworkStatus};
use super::protocol::{ClipboardMessage, PairingMessage, ProtocolMessage};

#[derive(Debug)]
pub enum NetworkCommand {
    StartListening,
    StopListening,
    SendPairingRequest {
        peer_id: String,
        message: Vec<u8>,
    },
    /// Send a pairing challenge (PIN) as a response to an incoming pairing request.
    /// The NetworkManager will look up the stored ResponseChannel for this peer.
    SendPairingChallenge {
        peer_id: String,
        session_id: String,
        pin: String,
        device_name: String,
    },
    /// Reject a pairing request
    RejectPairing {
        peer_id: String,
        session_id: String,
    },
    /// Send pairing confirmation (after PIN verification on initiator side)
    SendPairingConfirm {
        peer_id: String,
        session_id: String,
        success: bool,
        shared_secret: Option<Vec<u8>>,
        device_name: String,
    },
    BroadcastClipboard {
        message: ClipboardMessage,
    },
    GetPeers,
    /// Force reconnection to all discovered peers (used after app resume from background)
    ReconnectPeers,
}

/// Tracks retry state for a peer connection
#[derive(Debug, Clone)]
struct PeerRetryState {
    address: Multiaddr,
    retry_count: u32,
    next_retry: Instant,
}

pub struct NetworkManager {
    swarm: Swarm<DecentPasteBehaviour>,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
    discovered_peers: HashMap<PeerId, DiscoveredPeer>,
    connected_peers: HashMap<PeerId, ConnectedPeer>,
    pending_responses: HashMap<PeerId, ResponseChannel<ReqPairingResponse>>,
    /// Tracks peers that need connection retries
    pending_retries: HashMap<PeerId, PeerRetryState>,
}

impl NetworkManager {
    pub async fn new(
        command_rx: mpsc::Receiver<NetworkCommand>,
        event_tx: mpsc::Sender<NetworkEvent>,
        local_key: libp2p::identity::Keypair,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let local_peer_id = PeerId::from(local_key.public());
        info!("Local peer ID: {}", local_peer_id);

        // Create swarm
        let swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|_key| {
                DecentPasteBehaviour::new(local_peer_id, &local_key)
                    .expect("Failed to create behaviour")
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        Ok(Self {
            swarm,
            command_rx,
            event_tx,
            discovered_peers: HashMap::new(),
            connected_peers: HashMap::new(),
            pending_responses: HashMap::new(),
            pending_retries: HashMap::new(),
        })
    }

    pub fn local_peer_id(&self) -> String {
        self.swarm.local_peer_id().to_string()
    }

    pub async fn run(&mut self) {
        // Subscribe to clipboard topic
        if let Err(e) = self.swarm.behaviour_mut().subscribe_clipboard() {
            error!("Failed to subscribe to clipboard topic: {}", e);
        }

        // Start listening on all interfaces
        let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse().unwrap();
        if let Err(e) = self.swarm.listen_on(listen_addr) {
            error!("Failed to start listening: {}", e);
            let _ = self
                .event_tx
                .send(NetworkEvent::StatusChanged(NetworkStatus::Error(
                    e.to_string(),
                )))
                .await;
            return;
        }

        let _ = self
            .event_tx
            .send(NetworkEvent::StatusChanged(NetworkStatus::Connecting))
            .await;

        // Interval for processing connection retries
        let mut retry_interval = tokio::time::interval(Duration::from_millis(500));

        loop {
            tokio::select! {
                // Handle swarm events
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event).await;
                }

                // Handle commands
                Some(command) = self.command_rx.recv() => {
                    self.handle_command(command).await;
                }

                // Process pending retries
                _ = retry_interval.tick() => {
                    self.process_pending_retries();
                }
            }
        }
    }

    /// Process pending connection retries
    fn process_pending_retries(&mut self) {
        let now = Instant::now();
        let mut to_retry = Vec::new();

        // Find peers that are ready to retry
        for (peer_id, state) in &self.pending_retries {
            if now >= state.next_retry {
                to_retry.push((*peer_id, state.address.clone(), state.retry_count));
            }
        }

        // Process retries
        for (peer_id, addr, retry_count) in to_retry {
            // Remove from pending (will be re-added if it fails again)
            self.pending_retries.remove(&peer_id);

            // Skip if already connected
            if self.connected_peers.contains_key(&peer_id) {
                debug!("Skipping retry for {} - already connected", peer_id);
                continue;
            }

            info!(
                "Retrying connection to {} (attempt {}/{})",
                peer_id,
                retry_count + 1,
                MAX_CONNECTION_RETRIES
            );

            if let Err(e) = self.swarm.dial(addr) {
                warn!("Failed to initiate retry dial to {}: {}", peer_id, e);
            }
        }
    }

    async fn handle_swarm_event(
        &mut self,
        event: SwarmEvent<super::behaviour::DecentPasteBehaviourEvent>,
    ) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::StatusChanged(NetworkStatus::Connected))
                    .await;
            }

            SwarmEvent::Behaviour(super::behaviour::DecentPasteBehaviourEvent::Mdns(event)) => {
                match event {
                    mdns::Event::Discovered(peers) => {
                        for (peer_id, addr) in peers {
                            debug!("mDNS discovered: {} at {}", peer_id, addr);

                            // Add to dial queue
                            if let Err(e) = self.swarm.dial(addr.clone()) {
                                warn!("Failed to dial {}: {}", peer_id, e);
                            }

                            // Track discovered peer
                            let discovered = DiscoveredPeer {
                                peer_id: peer_id.to_string(),
                                device_name: None,
                                addresses: vec![addr.to_string()],
                                discovered_at: Utc::now(),
                                is_paired: false,
                            };

                            self.discovered_peers.insert(peer_id, discovered.clone());
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::PeerDiscovered(discovered))
                                .await;
                        }
                    }
                    mdns::Event::Expired(peers) => {
                        for (peer_id, _) in peers {
                            debug!("mDNS peer expired: {}", peer_id);
                            self.discovered_peers.remove(&peer_id);
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::PeerLost(peer_id.to_string()))
                                .await;
                        }
                    }
                }
            }

            SwarmEvent::Behaviour(super::behaviour::DecentPasteBehaviourEvent::Gossipsub(
                event,
            )) => match event {
                gossipsub::Event::Message { message, .. } => {
                    match ProtocolMessage::from_bytes(&message.data) {
                        Ok(ProtocolMessage::Clipboard(clipboard_msg)) => {
                            debug!(
                                "Received clipboard message from {}",
                                clipboard_msg.origin_device_id
                            );
                            let _ = self
                                .event_tx
                                .send(NetworkEvent::ClipboardReceived(clipboard_msg))
                                .await;
                        }
                        Ok(msg) => {
                            debug!("Received non-clipboard message via gossipsub: {:?}", msg);
                        }
                        Err(e) => {
                            warn!("Failed to parse gossipsub message: {}", e);
                        }
                    }
                }
                gossipsub::Event::Subscribed { peer_id, topic } => {
                    info!("Peer {} subscribed to topic {}", peer_id, topic);
                }
                gossipsub::Event::Unsubscribed { peer_id, topic } => {
                    info!("Peer {} unsubscribed from topic {}", peer_id, topic);
                }
                gossipsub::Event::GossipsubNotSupported { peer_id } => {
                    warn!("Peer {} does not support gossipsub", peer_id);
                }
                gossipsub::Event::SlowPeer { peer_id, .. } => {
                    warn!("Peer {} is slow", peer_id);
                }
            },

            SwarmEvent::Behaviour(
                super::behaviour::DecentPasteBehaviourEvent::RequestResponse(event),
            ) => {
                match event {
                    request_response::Event::Message {
                        peer,
                        connection_id: _connection_id,
                        message,
                    } => {
                        match message {
                            request_response::Message::Request {
                                request, channel, ..
                            } => {
                                debug!("Received pairing message from {}", peer);

                                // Parse the message
                                if let Ok(protocol_msg) =
                                    ProtocolMessage::from_bytes(&request.message)
                                {
                                    match protocol_msg {
                                        ProtocolMessage::Pairing(PairingMessage::Request(req)) => {
                                            // Store channel for later response
                                            self.pending_responses.insert(peer, channel);

                                            // Use the session_id from the initiator's request
                                            let session_id = req.session_id.clone();
                                            let _ = self
                                                .event_tx
                                                .send(NetworkEvent::PairingRequestReceived {
                                                    session_id,
                                                    peer_id: peer.to_string(),
                                                    request: req,
                                                })
                                                .await;
                                        }
                                        ProtocolMessage::Pairing(PairingMessage::Confirm(
                                            confirm,
                                        )) => {
                                            // Initiator sent confirmation after PIN verification
                                            // We (responder) need to complete pairing and send back acknowledgment
                                            debug!("Received pairing confirm from initiator: success={}", confirm.success);

                                            let initiator_device_name = confirm
                                                .device_name
                                                .clone()
                                                .unwrap_or_else(|| "Unknown Device".to_string());

                                            if confirm.success {
                                                if let Some(shared_secret) =
                                                    confirm.shared_secret.clone()
                                                {
                                                    // Send success acknowledgment back
                                                    let ack = super::protocol::PairingConfirm {
                                                        session_id: confirm.session_id.clone(),
                                                        success: true,
                                                        shared_secret: Some(shared_secret.clone()),
                                                        error: None,
                                                        device_name: None, // Not needed in ack
                                                    };
                                                    let ack_msg = ProtocolMessage::Pairing(
                                                        PairingMessage::Confirm(ack),
                                                    );
                                                    if let Ok(message) = ack_msg.to_bytes() {
                                                        let response =
                                                            ReqPairingResponse { message };
                                                        let _ = self
                                                            .swarm
                                                            .behaviour_mut()
                                                            .request_response
                                                            .send_response(channel, response);
                                                    }

                                                    // Emit pairing complete event for responder
                                                    let _ = self
                                                        .event_tx
                                                        .send(NetworkEvent::PairingComplete {
                                                            session_id: confirm.session_id,
                                                            peer_id: peer.to_string(),
                                                            device_name: initiator_device_name,
                                                            shared_secret,
                                                        })
                                                        .await;
                                                }
                                            } else {
                                                // Send failure acknowledgment
                                                let ack = super::protocol::PairingConfirm {
                                                    session_id: confirm.session_id.clone(),
                                                    success: false,
                                                    shared_secret: None,
                                                    error: confirm.error.clone(),
                                                    device_name: None,
                                                };
                                                let ack_msg = ProtocolMessage::Pairing(
                                                    PairingMessage::Confirm(ack),
                                                );
                                                if let Ok(message) = ack_msg.to_bytes() {
                                                    let response = ReqPairingResponse { message };
                                                    let _ = self
                                                        .swarm
                                                        .behaviour_mut()
                                                        .request_response
                                                        .send_response(channel, response);
                                                }

                                                let _ = self
                                                    .event_tx
                                                    .send(NetworkEvent::PairingFailed {
                                                        session_id: confirm.session_id,
                                                        error: confirm.error.unwrap_or_else(|| {
                                                            "Pairing cancelled".to_string()
                                                        }),
                                                    })
                                                    .await;
                                            }
                                        }
                                        _ => {
                                            debug!("Received unexpected pairing message type as request");
                                        }
                                    }
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                debug!("Received pairing response from {}", peer);
                                // Handle pairing response
                                if let Ok(protocol_msg) =
                                    ProtocolMessage::from_bytes(&response.message)
                                {
                                    if let ProtocolMessage::Pairing(pairing_msg) = protocol_msg {
                                        // Process pairing message
                                        match pairing_msg {
                                            PairingMessage::Challenge(challenge) => {
                                                let _ = self
                                                    .event_tx
                                                    .send(NetworkEvent::PairingPinReady {
                                                        session_id: challenge.session_id,
                                                        pin: challenge.pin,
                                                        peer_device_name: challenge.device_name,
                                                    })
                                                    .await;
                                            }
                                            PairingMessage::Confirm(confirm) => {
                                                if confirm.success {
                                                    if let Some(secret) = confirm.shared_secret {
                                                        let _ = self
                                                            .event_tx
                                                            .send(NetworkEvent::PairingComplete {
                                                                session_id: confirm.session_id,
                                                                peer_id: peer.to_string(),
                                                                device_name: "Unknown".to_string(),
                                                                shared_secret: secret,
                                                            })
                                                            .await;
                                                    }
                                                } else {
                                                    let _ = self
                                                        .event_tx
                                                        .send(NetworkEvent::PairingFailed {
                                                            session_id: confirm.session_id,
                                                            error: confirm.error.unwrap_or_else(
                                                                || "Unknown error".to_string(),
                                                            ),
                                                        })
                                                        .await;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }
                    }
                    request_response::Event::OutboundFailure { peer, error, .. } => {
                        warn!("Outbound request to {} failed: {}", peer, error);
                    }
                    request_response::Event::InboundFailure { peer, error, .. } => {
                        warn!("Inbound request from {} failed: {}", peer, error);
                    }
                    _ => {}
                }
            }

            SwarmEvent::Behaviour(super::behaviour::DecentPasteBehaviourEvent::Identify(event)) => {
                if let identify::Event::Received { peer_id, info, .. } = event {
                    debug!("Identified peer {}: {}", peer_id, info.agent_version);

                    // Update device name from identify info
                    if let Some(discovered) = self.discovered_peers.get_mut(&peer_id) {
                        discovered.device_name = Some(info.agent_version);
                    }
                }
            }

            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                debug!("Connection established with {}", peer_id);

                // Clear any pending retries for this peer
                self.pending_retries.remove(&peer_id);

                // Add peer to gossipsub mesh explicitly to ensure immediate message delivery
                // This is critical for reconnecting peers after restart
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .add_explicit_peer(&peer_id);
                debug!("Added {} to gossipsub explicit peers", peer_id);

                let connected = ConnectedPeer {
                    peer_id: peer_id.to_string(),
                    device_name: self
                        .discovered_peers
                        .get(&peer_id)
                        .and_then(|p| p.device_name.clone())
                        .unwrap_or_else(|| "Unknown".to_string()),
                    connected_at: Utc::now(),
                };
                self.connected_peers.insert(peer_id, connected.clone());
                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerConnected(connected))
                    .await;
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                debug!("Connection closed with {}", peer_id);

                // Remove peer from gossipsub explicit peers
                self.swarm
                    .behaviour_mut()
                    .gossipsub
                    .remove_explicit_peer(&peer_id);
                debug!("Removed {} from gossipsub explicit peers", peer_id);

                self.connected_peers.remove(&peer_id);
                let _ = self
                    .event_tx
                    .send(NetworkEvent::PeerDisconnected(peer_id.to_string()))
                    .await;
            }

            SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                warn!(
                    "Outgoing connection error to {:?}: {}",
                    peer_id, error
                );

                // Schedule retry if we have the peer's address and haven't exceeded max retries
                if let Some(peer_id) = peer_id {
                    if let Some(discovered) = self.discovered_peers.get(&peer_id) {
                        // Get current retry count
                        let current_retry = self
                            .pending_retries
                            .get(&peer_id)
                            .map(|s| s.retry_count)
                            .unwrap_or(0);

                        if current_retry < MAX_CONNECTION_RETRIES {
                            if let Ok(addr) = discovered.addresses[0].parse::<Multiaddr>() {
                                info!(
                                    "Scheduling retry {} for peer {} in {:?}",
                                    current_retry + 1,
                                    peer_id,
                                    RETRY_DELAY
                                );
                                self.pending_retries.insert(
                                    peer_id,
                                    PeerRetryState {
                                        address: addr,
                                        retry_count: current_retry + 1,
                                        next_retry: Instant::now() + RETRY_DELAY,
                                    },
                                );
                            }
                        } else {
                            warn!(
                                "Max retries ({}) exceeded for peer {}",
                                MAX_CONNECTION_RETRIES, peer_id
                            );
                            self.pending_retries.remove(&peer_id);
                        }
                    }
                }
            }

            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }

            SwarmEvent::Dialing { peer_id, .. } => {
                info!("Dialing peer: {:?}", peer_id);
            }

            _ => {}
        }
    }

    async fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::BroadcastClipboard { message } => {
                let protocol_msg = ProtocolMessage::Clipboard(message.clone());
                match self.swarm.behaviour_mut().publish_clipboard(&protocol_msg) {
                    Ok(_) => {
                        debug!("Broadcast clipboard message: {}", message.id);
                        let _ = self
                            .event_tx
                            .send(NetworkEvent::ClipboardSent {
                                id: message.id,
                                peer_count: self.connected_peers.len(),
                            })
                            .await;
                    }
                    Err(e) => {
                        warn!("Failed to broadcast clipboard: {}", e);
                    }
                }
            }

            NetworkCommand::SendPairingRequest { peer_id, message } => {
                if let Ok(peer) = peer_id.parse::<PeerId>() {
                    let request = ReqPairingRequest { message };
                    self.swarm
                        .behaviour_mut()
                        .request_response
                        .send_request(&peer, request);
                    debug!("Sent pairing request to {}", peer_id);
                }
            }

            NetworkCommand::SendPairingChallenge {
                peer_id,
                session_id,
                pin,
                device_name,
            } => {
                if let Ok(peer) = peer_id.parse::<PeerId>() {
                    if let Some(channel) = self.pending_responses.remove(&peer) {
                        let challenge = super::protocol::PairingChallenge {
                            session_id: session_id.clone(),
                            pin,
                            device_name,
                        };
                        let protocol_msg =
                            ProtocolMessage::Pairing(PairingMessage::Challenge(challenge));
                        if let Ok(message) = protocol_msg.to_bytes() {
                            let response = ReqPairingResponse { message };
                            if self
                                .swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response)
                                .is_ok()
                            {
                                debug!("Sent pairing challenge to {}", peer_id);
                            } else {
                                warn!("Failed to send pairing challenge to {}", peer_id);
                            }
                        }
                    } else {
                        warn!("No pending response channel for peer {}", peer_id);
                    }
                }
            }

            NetworkCommand::RejectPairing {
                peer_id,
                session_id,
            } => {
                if let Ok(peer) = peer_id.parse::<PeerId>() {
                    if let Some(channel) = self.pending_responses.remove(&peer) {
                        let confirm = super::protocol::PairingConfirm {
                            session_id,
                            success: false,
                            shared_secret: None,
                            error: Some("Pairing rejected by user".to_string()),
                            device_name: None,
                        };
                        let protocol_msg =
                            ProtocolMessage::Pairing(PairingMessage::Confirm(confirm));
                        if let Ok(message) = protocol_msg.to_bytes() {
                            let response = ReqPairingResponse { message };
                            let _ = self
                                .swarm
                                .behaviour_mut()
                                .request_response
                                .send_response(channel, response);
                            debug!("Sent pairing rejection to {}", peer_id);
                        }
                    }
                }
            }

            NetworkCommand::SendPairingConfirm {
                peer_id,
                session_id,
                success,
                shared_secret,
                device_name,
            } => {
                // This is sent as a NEW request from initiator to responder after PIN confirmation
                if let Ok(peer) = peer_id.parse::<PeerId>() {
                    let confirm = super::protocol::PairingConfirm {
                        session_id,
                        success,
                        shared_secret,
                        error: None,
                        device_name: Some(device_name),
                    };
                    let protocol_msg = ProtocolMessage::Pairing(PairingMessage::Confirm(confirm));
                    if let Ok(message) = protocol_msg.to_bytes() {
                        let request = ReqPairingRequest { message };
                        self.swarm
                            .behaviour_mut()
                            .request_response
                            .send_request(&peer, request);
                        debug!("Sent pairing confirm to {}", peer_id);
                    }
                }
            }

            NetworkCommand::GetPeers => {
                // Send current peer lists
                for peer in self.discovered_peers.values() {
                    let _ = self
                        .event_tx
                        .send(NetworkEvent::PeerDiscovered(peer.clone()))
                        .await;
                }
            }

            NetworkCommand::ReconnectPeers => {
                info!("Reconnecting to all discovered peers (app resumed from background)");

                // Clear stale connection state
                self.connected_peers.clear();
                self.pending_retries.clear();

                // Try to dial all discovered peers
                for (peer_id, peer) in &self.discovered_peers {
                    if let Some(addr_str) = peer.addresses.first() {
                        if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                            info!("Attempting to reconnect to {} at {}", peer_id, addr);
                            if let Err(e) = self.swarm.dial(addr) {
                                warn!("Failed to initiate reconnection to {}: {}", peer_id, e);
                            }
                        }
                    }
                }
            }

            NetworkCommand::StartListening | NetworkCommand::StopListening => {
                // Already handled during initialization
            }
        }
    }
}
