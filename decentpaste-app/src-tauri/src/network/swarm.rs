use chrono::Utc;
use futures::StreamExt;
use libp2p::{
    gossipsub, identify, mdns,
    noise, tcp, yamux,
    request_response::{self, ResponseChannel},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::behaviour::{DecentPasteBehaviour, PairingRequest as ReqPairingRequest, PairingResponse as ReqPairingResponse};
use super::events::{ConnectedPeer, DiscoveredPeer, NetworkEvent, NetworkStatus};
use super::protocol::{ProtocolMessage, ClipboardMessage, PairingMessage};

#[derive(Debug)]
pub enum NetworkCommand {
    StartListening,
    StopListening,
    SendPairingRequest {
        peer_id: String,
        message: Vec<u8>,
    },
    SendPairingResponse {
        peer_id: String,
        channel: ResponseChannel<ReqPairingResponse>,
        message: Vec<u8>,
    },
    BroadcastClipboard {
        message: ClipboardMessage,
    },
    GetPeers,
}

pub struct NetworkManager {
    swarm: Swarm<DecentPasteBehaviour>,
    command_rx: mpsc::Receiver<NetworkCommand>,
    event_tx: mpsc::Sender<NetworkEvent>,
    discovered_peers: HashMap<PeerId, DiscoveredPeer>,
    connected_peers: HashMap<PeerId, ConnectedPeer>,
    pending_responses: HashMap<PeerId, ResponseChannel<ReqPairingResponse>>,
}

impl NetworkManager {
    pub async fn new(
        command_rx: mpsc::Receiver<NetworkCommand>,
        event_tx: mpsc::Sender<NetworkEvent>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create identity
        let local_key = libp2p::identity::Keypair::generate_ed25519();
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
            .with_swarm_config(|cfg| {
                cfg.with_idle_connection_timeout(Duration::from_secs(60))
            })
            .build();

        Ok(Self {
            swarm,
            command_rx,
            event_tx,
            discovered_peers: HashMap::new(),
            connected_peers: HashMap::new(),
            pending_responses: HashMap::new(),
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
            let _ = self.event_tx.send(NetworkEvent::StatusChanged(NetworkStatus::Error(e.to_string()))).await;
            return;
        }

        let _ = self.event_tx.send(NetworkEvent::StatusChanged(NetworkStatus::Connecting)).await;

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
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<super::behaviour::DecentPasteBehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}", address);
                let _ = self.event_tx.send(NetworkEvent::StatusChanged(NetworkStatus::Connected)).await;
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
                            let _ = self.event_tx.send(NetworkEvent::PeerDiscovered(discovered)).await;
                        }
                    }
                    mdns::Event::Expired(peers) => {
                        for (peer_id, _) in peers {
                            debug!("mDNS peer expired: {}", peer_id);
                            self.discovered_peers.remove(&peer_id);
                            let _ = self.event_tx.send(NetworkEvent::PeerLost(peer_id.to_string())).await;
                        }
                    }
                }
            }

            SwarmEvent::Behaviour(super::behaviour::DecentPasteBehaviourEvent::Gossipsub(event)) => {
                if let gossipsub::Event::Message { message, .. } = event {
                    match ProtocolMessage::from_bytes(&message.data) {
                        Ok(ProtocolMessage::Clipboard(clipboard_msg)) => {
                            debug!("Received clipboard message from {}", clipboard_msg.origin_device_id);
                            let _ = self.event_tx.send(NetworkEvent::ClipboardReceived(clipboard_msg)).await;
                        }
                        Ok(msg) => {
                            debug!("Received non-clipboard message via gossipsub: {:?}", msg);
                        }
                        Err(e) => {
                            warn!("Failed to parse gossipsub message: {}", e);
                        }
                    }
                }
            }

            SwarmEvent::Behaviour(super::behaviour::DecentPasteBehaviourEvent::RequestResponse(event)) => {
                match event {
                    request_response::Event::Message { peer, message } => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                debug!("Received pairing request from {}", peer);
                                self.pending_responses.insert(peer, channel);

                                // Parse the message
                                if let Ok(protocol_msg) = ProtocolMessage::from_bytes(&request.message) {
                                    if let ProtocolMessage::Pairing(PairingMessage::Request(req)) = protocol_msg {
                                        let session_id = uuid::Uuid::new_v4().to_string();
                                        let _ = self.event_tx.send(NetworkEvent::PairingRequestReceived {
                                            session_id,
                                            peer_id: peer.to_string(),
                                            request: req,
                                        }).await;
                                    }
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                debug!("Received pairing response from {}", peer);
                                // Handle pairing response
                                if let Ok(protocol_msg) = ProtocolMessage::from_bytes(&response.message) {
                                    if let ProtocolMessage::Pairing(pairing_msg) = protocol_msg {
                                        // Process pairing message
                                        match pairing_msg {
                                            PairingMessage::Challenge(challenge) => {
                                                let _ = self.event_tx.send(NetworkEvent::PairingPinReady {
                                                    session_id: challenge.session_id,
                                                    pin: challenge.pin,
                                                }).await;
                                            }
                                            PairingMessage::Confirm(confirm) => {
                                                if confirm.success {
                                                    if let Some(secret) = confirm.shared_secret {
                                                        let _ = self.event_tx.send(NetworkEvent::PairingComplete {
                                                            session_id: confirm.session_id,
                                                            peer_id: peer.to_string(),
                                                            device_name: "Unknown".to_string(),
                                                            shared_secret: secret,
                                                        }).await;
                                                    }
                                                } else {
                                                    let _ = self.event_tx.send(NetworkEvent::PairingFailed {
                                                        session_id: confirm.session_id,
                                                        error: confirm.error.unwrap_or_else(|| "Unknown error".to_string()),
                                                    }).await;
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
                let connected = ConnectedPeer {
                    peer_id: peer_id.to_string(),
                    device_name: self.discovered_peers
                        .get(&peer_id)
                        .and_then(|p| p.device_name.clone())
                        .unwrap_or_else(|| "Unknown".to_string()),
                    connected_at: Utc::now(),
                };
                self.connected_peers.insert(peer_id, connected.clone());
                let _ = self.event_tx.send(NetworkEvent::PeerConnected(connected)).await;
            }

            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                debug!("Connection closed with {}", peer_id);
                self.connected_peers.remove(&peer_id);
                let _ = self.event_tx.send(NetworkEvent::PeerDisconnected(peer_id.to_string())).await;
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
                        let _ = self.event_tx.send(NetworkEvent::ClipboardSent {
                            id: message.id,
                            peer_count: self.connected_peers.len(),
                        }).await;
                    }
                    Err(e) => {
                        warn!("Failed to broadcast clipboard: {}", e);
                    }
                }
            }

            NetworkCommand::SendPairingRequest { peer_id, message } => {
                if let Ok(peer) = peer_id.parse::<PeerId>() {
                    let request = ReqPairingRequest { message };
                    self.swarm.behaviour_mut().request_response.send_request(&peer, request);
                    debug!("Sent pairing request to {}", peer_id);
                }
            }

            NetworkCommand::SendPairingResponse { peer_id, channel, message } => {
                let response = ReqPairingResponse { message };
                let _ = self.swarm.behaviour_mut().request_response.send_response(channel, response);
                debug!("Sent pairing response to {}", peer_id);
            }

            NetworkCommand::GetPeers => {
                // Send current peer lists
                for peer in self.discovered_peers.values() {
                    let _ = self.event_tx.send(NetworkEvent::PeerDiscovered(peer.clone())).await;
                }
            }

            NetworkCommand::StartListening | NetworkCommand::StopListening => {
                // Already handled during initialization
            }
        }
    }
}
