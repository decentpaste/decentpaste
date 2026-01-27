//! DecentPaste Relay Server
//!
//! A libp2p relay server that enables DecentPaste clients behind NAT to connect
//! to each other. The relay cannot decrypt clipboard content - it only forwards
//! encrypted bytes between peers.
//!
//! # Security Properties
//!
//! - E2E encryption: All clipboard content is AES-256-GCM encrypted before reaching relay
//! - No content access: Relay sees only encrypted bytes and metadata (PeerIds, timing)
//! - Privacy: PeerIds are pseudonymous (derived from Ed25519 public keys)
//!
//! # Rate Limiting
//!
//! - Max circuit reservations per peer: 10
//! - Max circuit duration: 30 minutes
//! - Max bandwidth per circuit: 128 KB/s (clipboard sync doesn't need more)
//!
//! # Identity Persistence
//!
//! The relay's keypair is persisted to disk so the PeerId remains stable across restarts.
//! This is important because clients embed the relay's PeerId in pairing codes.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use futures::StreamExt;
use libp2p::{
    identify, noise, relay,
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId,
};
use tracing::{debug, info, warn};

/// DecentPaste Relay Server
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// Port to listen on for libp2p connections
    #[arg(short, long, default_value = "4001")]
    port: u16,

    /// Port for HTTP health check endpoint
    #[arg(long, default_value = "8080")]
    health_port: u16,

    /// Maximum number of circuit reservations
    #[arg(long, default_value = "1000")]
    max_reservations: usize,

    /// Maximum circuit duration in seconds
    #[arg(long, default_value = "1800")]
    max_circuit_duration_secs: u64,

    /// Path to the keypair file (will be created if it doesn't exist)
    #[arg(long, default_value = "relay_key")]
    key_file: PathBuf,

    /// External IP address of this relay server (required for NAT traversal).
    /// Clients use this address to connect through the relay.
    /// Example: --external-ip xx.xx.xx.xx
    #[arg(long)]
    external_ip: Option<String>,
}

/// Load or generate the relay's keypair.
///
/// If the key file exists, loads the keypair from it.
/// Otherwise, generates a new Ed25519 keypair and saves it.
fn load_or_generate_keypair(key_path: &PathBuf) -> Result<libp2p::identity::Keypair> {
    if key_path.exists() {
        // Load existing keypair
        let key_data = std::fs::read(key_path)
            .with_context(|| format!("Failed to read keypair from {:?}", key_path))?;
        let keypair = libp2p::identity::Keypair::from_protobuf_encoding(&key_data)
            .with_context(|| "Failed to decode keypair from protobuf")?;
        info!("Loaded existing keypair from {:?}", key_path);
        Ok(keypair)
    } else {
        // Generate new keypair
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let key_data = keypair
            .to_protobuf_encoding()
            .with_context(|| "Failed to encode keypair to protobuf")?;
        std::fs::write(key_path, &key_data)
            .with_context(|| format!("Failed to write keypair to {:?}", key_path))?;
        info!("Generated new keypair and saved to {:?}", key_path);
        Ok(keypair)
    }
}

/// The network behaviour for the relay server
#[derive(NetworkBehaviour)]
struct RelayServerBehaviour {
    /// The relay server behaviour (accepts reservations, forwards circuits)
    relay: relay::Behaviour,
    /// Identify behaviour for peer identification
    identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("decentpaste_relay=info".parse()?)
                .add_directive("libp2p=info".parse()?),
        )
        .init();

    let args = Args::parse();

    info!("Starting DecentPaste Relay Server");
    info!("libp2p port: {}", args.port);
    info!("Health check port: {}", args.health_port);

    // Load or generate the relay's identity (persisted to disk)
    let keypair = load_or_generate_keypair(&args.key_file)?;
    let local_peer_id = PeerId::from(keypair.public());
    info!("Relay Peer ID: {}", local_peer_id);
    info!("  (keypair stored at: {:?})", args.key_file);

    // Configure relay limits
    let relay_config = relay::Config {
        max_reservations: args.max_reservations,
        max_reservations_per_peer: 10,
        max_circuits: args.max_reservations * 2, // Each reservation can have multiple circuits
        max_circuits_per_peer: 20,
        reservation_duration: Duration::from_secs(args.max_circuit_duration_secs),
        ..Default::default()
    };

    // Create the swarm
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(keypair.clone())
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )?
        .with_behaviour(|key| {
            let relay = relay::Behaviour::new(local_peer_id, relay_config);
            let identify = identify::Behaviour::new(
                identify::Config::new("/decentpaste-relay/1.0.0".to_string(), key.public())
                    .with_agent_version(format!(
                        "decentpaste-relay/{}",
                        env!("CARGO_PKG_VERSION")
                    )),
            );
            RelayServerBehaviour { relay, identify }
        })?
        .with_swarm_config(|cfg| {
            // Match reservation duration to prevent connections from being dropped
            // before the reservation expires. Default reservation is 30 minutes.
            cfg.with_idle_connection_timeout(Duration::from_secs(args.max_circuit_duration_secs))
        })
        .build();

    // Listen on all interfaces
    let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", args.port).parse()?;
    swarm.listen_on(listen_addr)?;

    // Also listen on IPv6 if available
    let listen_addr_v6: Multiaddr = format!("/ip6/::/tcp/{}", args.port).parse()?;
    if let Err(e) = swarm.listen_on(listen_addr_v6) {
        warn!("Could not listen on IPv6: {}", e);
    }

    // Add external address so relay reservations include reachable addresses.
    // Without this, clients get NoAddressesInReservation error because 0.0.0.0
    // is not a valid external address.
    if let Some(ref external_ip) = args.external_ip {
        let external_addr: Multiaddr = format!("/ip4/{}/tcp/{}", external_ip, args.port).parse()?;
        swarm.add_external_address(external_addr.clone());
        info!("Added external address: {}", external_addr);
    } else {
        warn!("No --external-ip specified. Relay reservations will fail with NoAddressesInReservation.");
        warn!("Use --external-ip <YOUR_PUBLIC_IP> to enable relay functionality.");
    }

    // Start health check HTTP server
    let health_addr: SocketAddr = format!("0.0.0.0:{}", args.health_port).parse()?;
    let health_peer_id = local_peer_id.to_string();
    tokio::spawn(async move {
        run_health_server(health_addr, health_peer_id).await;
    });

    info!("Relay server started, waiting for connections...");

    // Main event loop
    loop {
        match swarm.select_next_some().await {
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {}/p2p/{}", address, local_peer_id);
            }

            SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                info!(
                    "Connection established with {} via {:?}",
                    peer_id,
                    endpoint.get_remote_address()
                );
            }

            SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                // Log at INFO level so we can see connection drops during debugging
                info!("Connection closed with {}: {:?}", peer_id, cause);
            }

            SwarmEvent::Behaviour(RelayServerBehaviourEvent::Relay(event)) => {
                match event {
                    relay::Event::ReservationReqAccepted { src_peer_id, .. } => {
                        info!("Accepted relay reservation from {}", src_peer_id);
                    }
                    relay::Event::ReservationReqDenied { src_peer_id } => {
                        warn!("Denied relay reservation from {} (rate limited)", src_peer_id);
                    }
                    relay::Event::ReservationTimedOut { src_peer_id } => {
                        info!("Relay reservation timed out for {}", src_peer_id);
                    }
                    relay::Event::CircuitReqAccepted { src_peer_id, dst_peer_id, .. } => {
                        info!(
                            "Circuit established: {} -> {}",
                            src_peer_id, dst_peer_id
                        );
                    }
                    relay::Event::CircuitReqDenied { src_peer_id, dst_peer_id } => {
                        warn!(
                            "Circuit denied: {} -> {} (rate limited or not found)",
                            src_peer_id, dst_peer_id
                        );
                    }
                    relay::Event::CircuitClosed { src_peer_id, dst_peer_id, .. } => {
                        debug!("Circuit closed: {} -> {}", src_peer_id, dst_peer_id);
                    }
                    _ => {}
                }
            }

            SwarmEvent::Behaviour(RelayServerBehaviourEvent::Identify(event)) => {
                if let identify::Event::Received { peer_id, info, .. } = event {
                    debug!(
                        "Identified peer {}: {} ({})",
                        peer_id, info.agent_version, info.protocol_version
                    );
                }
            }

            SwarmEvent::IncomingConnectionError { error, .. } => {
                warn!("Incoming connection error: {}", error);
            }

            _ => {}
        }
    }
}

/// Run a simple HTTP health check server
async fn run_health_server(addr: SocketAddr, peer_id: String) {
    use axum::{routing::get, Json, Router};
    use serde_json::json;

    let app = Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status": "healthy"})) }),
        )
        .route(
            "/info",
            get(move || {
                let pid = peer_id.clone();
                async move {
                    Json(json!({
                        "peer_id": pid,
                        "version": env!("CARGO_PKG_VERSION"),
                    }))
                }
            }),
        );

    info!("Health check server listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    if let Err(e) = axum::serve(listener, app).await {
        warn!("Health check server error: {}", e);
    }
}
