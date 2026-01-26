//! Coordinator - Minimal peer lifecycle manager
//!
//! The Coordinator handles:
//! - Peer lifecycle (Connected/Disconnected events from transport)
//! - Spawning PeerActors on new connections
//! - Authentication state tracking
//! - Cross-peer operations (relay, broadcast datagrams)
//!
//! All protocol handling is done by PeerActor. App communicates directly with PeerActor
//! for operations like publishing, subscribing, etc.

mod state;

use std::sync::Arc;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::{debug, error, info, warn};
use transport::{ConnectionHandle, NodeId, TransportEvent};

use tokio::sync::{mpsc, oneshot};

use crate::handle::CourierEvent;
use crate::peer_actor::{PeerActor, PeerActorArgs, PeerMessage};
use crate::state::PeerType;
use butler::Butler;

pub use state::{CoordinatorState, PeerEntry, PeerInfo};

/// Request for CourierRunner to initiate a connection
#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub node_id: NodeId,
    pub permit: String,
}

/// Messages received by Coordinator
#[derive(Debug)]
pub enum CoordinatorMessage {
    // ==================== Lifecycle ====================

    /// New connection from transport
    Connected {
        node_id: NodeId,
        conn: ConnectionHandle,
    },

    /// Connection disconnected
    Disconnected { node_id: NodeId },

    /// PeerActor completed handshake
    PeerAuthenticated {
        node_id: NodeId,
        peer_type: PeerType,
        did: String,
        username: String,
    },

    /// PeerActor failed
    PeerFailed { node_id: NodeId, reason: String },

    // ==================== Connection Management ====================

    /// Connect to peer (fire-and-forget, result via CourierEvent)
    ///
    /// **Context**: App wants to connect to a peer
    /// **We do**: Store permit, mark as pending, request transport connection
    /// **Events emitted**: PeerAuthenticated on success, ConnectionFailed on failure
    Connect {
        node_id: NodeId,
        permit: String,
    },

    /// Get PeerActor reference for direct communication
    GetPeerActor {
        node_id: NodeId,
        response: oneshot::Sender<Option<ActorRef<PeerMessage>>>,
    },

    // ==================== Cross-Peer Operations ====================

    /// Relay datagram to other peers (Node mode only)
    RelayDatagram {
        from_node_id: NodeId,
        data: Vec<u8>,
    },

    /// Broadcast datagram to all authenticated peers
    BroadcastDatagram { data: Vec<u8> },

    // ==================== Scribe Integration ====================

    /// Scribe requested sync with a user
    EnsureSync { user_did: String },

    // ==================== Control ====================

    /// Shutdown all actors
    Shutdown,
}

/// Courier mode (User or Node)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourierMode {
    /// Running as user's device (connects TO node)
    User,
    /// Running as always-on node (accepts connections)
    Node,
}

/// Coordinator manages all PeerActors
pub struct Coordinator;

impl Coordinator {
    pub fn new() -> Self {
        Self
    }

    /// Emit an event to the app layer via event_tx channel
    fn emit_event(state: &CoordinatorState, event: CourierEvent) {
        if let Some(tx) = &state.event_tx {
            if let Err(e) = tx.try_send(event) {
                warn!("Failed to emit event: {}", e);
            }
        }
    }

    /// Process a TransportEvent and convert to CoordinatorMessage
    ///
    /// **Note**: Transport now only emits lifecycle events (Connected/Disconnected).
    /// Message reading is done by PeerSession directly from the ConnectionHandle.
    pub fn from_transport_event(event: TransportEvent) -> Option<CoordinatorMessage> {
        match event {
            TransportEvent::Connected { node_id, conn } => {
                Some(CoordinatorMessage::Connected { node_id, conn })
            }
            TransportEvent::Disconnected { node_id } => {
                Some(CoordinatorMessage::Disconnected { node_id })
            }
            // Note: Bytes and Error events removed - PeerSession handles reading now
        }
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(feature = "async-trait", ractor::async_trait)]
impl Actor for Coordinator {
    type Msg = CoordinatorMessage;
    type State = CoordinatorState;
    type Arguments = (
        NodeId,
        CourierMode,
        Arc<Butler>,
        crate::peer_actor::BlobStore,
        Option<mpsc::Sender<ConnectRequest>>,
        Option<mpsc::Sender<CourierEvent>>,
    );

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (our_node_id, mode, butler, blob_store, connect_tx, event_tx) = args;

        info!("Coordinator started in {:?} mode (node_id={})", mode, our_node_id);

        Ok(CoordinatorState::new(
            our_node_id,
            mode,
            butler,
            blob_store,
            connect_tx,
            event_tx,
        ))
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            // ==================== Lifecycle ====================

            CoordinatorMessage::Connected { node_id, conn } => {
                self.on_connected(myself.clone(), node_id, conn, state).await;
            }

            CoordinatorMessage::Disconnected { node_id } => {
                self.on_disconnected(node_id, state).await;
            }

            CoordinatorMessage::PeerAuthenticated {
                node_id,
                peer_type,
                did,
                username,
            } => {
                info!("Peer authenticated: {} ({}) - {:?}", username, node_id, peer_type);
                state.authenticate_peer(node_id, peer_type, did.clone(), username.clone());

                // Mark sovereign node as connected (for User mode connecting to their Node)
                if peer_type == PeerType::MyNode {
                    let node_id_str = node_id.to_string();
                    match state.butler.set_sovereign_node_connected(&node_id_str, true) {
                        Ok(true) => info!("Marked sovereign node {} as connected", node_id_str),
                        Ok(false) => debug!("Sovereign node {} not found in storage", node_id_str),
                        Err(e) => warn!("Failed to mark sovereign node connected: {}", e),
                    }
                }

                // Emit event for app layer
                Self::emit_event(state, CourierEvent::PeerAuthenticated {
                    node_id: node_id.to_string(),
                    did,
                    username,
                });
            }

            CoordinatorMessage::PeerFailed { node_id, reason } => {
                warn!("Peer failed: {} - {}", node_id, reason);

                // Emit failure event for app layer
                Self::emit_event(state, CourierEvent::ConnectionFailed {
                    node_id: node_id.to_string(),
                    error: reason,
                });

                state.remove_peer(node_id);
            }

            // ==================== Connection Management ====================

            CoordinatorMessage::Connect { node_id, permit } => {
                self.on_connect(node_id, permit, state).await;
            }

            CoordinatorMessage::GetPeerActor { node_id, response } => {
                let actor = state.get_actor(&node_id).cloned();
                let _ = response.send(actor);
            }

            // ==================== Cross-Peer Operations ====================

            CoordinatorMessage::RelayDatagram { from_node_id, data } => {
                if state.mode == CourierMode::Node {
                    for (peer_node_id, entry) in state.authenticated_peers() {
                        if peer_node_id != from_node_id {
                            if let Err(e) = entry.conn.send_datagram(&data) {
                                debug!(from = %from_node_id, to = %peer_node_id, error = %e, "Failed to relay datagram");
                            }
                        }
                    }
                }
            }

            CoordinatorMessage::BroadcastDatagram { data } => {
                for (node_id, entry) in state.authenticated_peers() {
                    if let Err(e) = entry.conn.send_datagram(&data) {
                        debug!(node_id = %node_id, error = %e, "Failed to send datagram");
                    }
                }
            }

            // ==================== Scribe Integration ====================

            CoordinatorMessage::EnsureSync { user_did } => {
                self.on_ensure_sync(myself.clone(), user_did, state).await;
            }

            // ==================== Control ====================

            CoordinatorMessage::Shutdown => {
                info!("Coordinator shutting down");
                for (node_id, entry) in state.peers.drain() {
                    debug!("Stopping PeerActor {}", node_id);
                    entry.actor.stop(Some("coordinator shutdown".to_string()));
                }
                myself.stop(Some("shutdown".to_string()));
            }
        }

        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        _myself: ActorRef<Self::Msg>,
        event: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match event {
            SupervisionEvent::ActorStarted(cell) => {
                debug!("Child actor started: {:?}", cell.get_id());
            }
            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                debug!(
                    "Child actor {} terminated: {:?}",
                    cell.get_id(),
                    reason
                );
                // Find and remove from unified registry
                state.peers.retain(|node_id, entry| {
                    if entry.actor.get_id() == cell.get_id() {
                        debug!("Removing terminated PeerActor {}", node_id);
                        false
                    } else {
                        true
                    }
                });
            }
            SupervisionEvent::ActorFailed(cell, error) => {
                error!("Child actor {} failed: {}", cell.get_id(), error);
                // Same cleanup as terminated
                state.peers.retain(|node_id, entry| {
                    if entry.actor.get_id() == cell.get_id() {
                        debug!("Removing failed PeerActor {}", node_id);
                        false
                    } else {
                        true
                    }
                });
            }
            _ => {}
        }

        Ok(())
    }
}

impl Coordinator {
    /// Handle new connection - spawn PeerActor with permit if outbound
    async fn on_connected(
        &self,
        myself: ActorRef<CoordinatorMessage>,
        node_id: NodeId,
        conn: ConnectionHandle,
        state: &mut CoordinatorState,
    ) {
        info!("New connection from: {}", node_id);

        // Check if this was an outbound connection WE initiated - get permit if so
        let is_outbound = state.pending_connections.remove(&node_id);
        let permit = if is_outbound {
            state.pending_permits.remove(&node_id)
        } else {
            None
        };

        if is_outbound && permit.is_none() {
            warn!("Outbound connection to {} but no permit - cannot handshake", node_id);
        }

        // Spawn PeerActor - if permit provided, it auto-initiates handshake
        let peer_actor = PeerActor::new(node_id);
        let args = PeerActorArgs {
            mode: state.mode,
            conn: conn.clone(),
            coordinator: myself.clone(),
            butler: state.butler.clone(),
            blob_store: state.blob_store.clone(),
            permit,
        };

        match Actor::spawn_linked(None, peer_actor, args, myself.get_cell()).await {
            Ok((actor_ref, _)) => {
                state.add_peer(node_id, actor_ref, conn);
                debug!("PeerActor spawned for {}", node_id);
            }
            Err(e) => {
                error!("Failed to spawn PeerActor for {}: {:?}", node_id, e);
            }
        }
    }

    /// Handle disconnection - cleanup peer state and emit event
    async fn on_disconnected(&self, node_id: NodeId, state: &mut CoordinatorState) {
        info!("Disconnected: {}", node_id);

        if let Some(entry) = state.peers.remove(&node_id) {
            entry.actor.stop(Some("disconnected".to_string()));
        }

        state.pending_connections.remove(&node_id);
        state.pending_permits.remove(&node_id);

        // Emit disconnect event for app layer
        Self::emit_event(state, CourierEvent::PeerDisconnected {
            node_id: node_id.to_string(),
        });
    }

    /// Connect to peer (fire-and-forget, result via events)
    ///
    /// **Context**: App calls connect(), we store permit and mark as pending
    /// **Transport**: CourierRunner sees pending_connections and calls transport.connect()
    /// **Events**: PeerAuthenticated on success, ConnectionFailed on failure
    async fn on_connect(
        &self,
        node_id: NodeId,
        permit: String,
        state: &mut CoordinatorState,
    ) {
        info!("Connect: node={}", node_id);

        // Check if already authenticated - emit event immediately
        if state.is_authenticated(&node_id) {
            info!("Peer {} already authenticated", node_id);
            if let Some(auth) = state.get_auth(&node_id) {
                Self::emit_event(state, CourierEvent::PeerAuthenticated {
                    node_id: node_id.to_string(),
                    did: auth.did.clone(),
                    username: auth.username.clone(),
                });
            }
            return;
        }

        // Check if already connected - initiate handshake with existing PeerActor
        if let Some(actor) = state.get_actor(&node_id).cloned() {
            info!("Peer {} already connected - initiating handshake", node_id);
            if let Err(e) = actor.cast(PeerMessage::InitiateHandshake { permit }) {
                warn!("Failed to send InitiateHandshake to {}: {:?}", node_id, e);
            }
            return;
        }

        // Mark as pending outbound connection and store permit
        state.pending_connections.insert(node_id);
        state.pending_permits.insert(node_id, permit.clone());

        // Request transport connection via CourierRunner
        if let Some(ref tx) = state.connect_tx {
            if let Err(e) = tx.try_send(ConnectRequest { node_id, permit }) {
                warn!(node_id = %node_id, error = %e, "Failed to send connect request");
                Self::emit_event(state, CourierEvent::ConnectionFailed {
                    node_id: node_id.to_string(),
                    error: format!("Failed to initiate connection: {}", e),
                });
            }
        }
    }

    /// Handle EnsureSync from Scribe - connect to user if not already connected
    async fn on_ensure_sync(
        &self,
        _myself: ActorRef<CoordinatorMessage>,
        user_did: String,
        state: &mut CoordinatorState,
    ) {
        info!(user_did = %user_did, "Scribe requested sync with user");

        // Check if user is already connected
        if let Some(peer_actor) = self.get_peer_actor_for_user(&user_did, state) {
            debug!(user_did = %user_did, "User already connected, refreshing subscriptions");
            let _ = peer_actor.cast(PeerMessage::RefreshSubscriptions);
            return;
        }

        // Resolve user_did to device info
        let device_info = match state.butler.resolve_device_for_user(&user_did) {
            Ok(Some(info)) => info,
            Ok(None) => {
                warn!(user_did = %user_did, "No device info found for user");
                return;
            }
            Err(e) => {
                warn!(user_did = %user_did, error = %e, "Failed to resolve device");
                return;
            }
        };

        let node_id = match device_info.node_id.parse::<NodeId>() {
            Ok(id) => id,
            Err(e) => {
                warn!(node_id = %device_info.node_id, error = ?e, "Failed to parse node_id");
                return;
            }
        };

        // Check if already connecting/connected
        if state.is_connected(&node_id) || state.pending_connections.contains(&node_id) {
            debug!(user_did = %user_did, node_id = %node_id, "Already connected/connecting");
            return;
        }

        info!(user_did = %user_did, node_id = %node_id, "Requesting connection for sync");

        state.pending_connections.insert(node_id);
        state.pending_permits.insert(node_id, device_info.permit.clone());

        if let Some(ref tx) = state.connect_tx {
            if let Err(e) = tx.try_send(ConnectRequest {
                node_id,
                permit: device_info.permit.clone(),
            }) {
                warn!(node_id = %node_id, error = %e, "Failed to send connect request");
            }
        }

        Self::emit_event(state, CourierEvent::ConnectRequested {
            node_id: node_id.to_string(),
            permit: device_info.permit,
        });
    }

    /// Find peer actor for a user by DID
    fn get_peer_actor_for_user<'a>(
        &self,
        user_did: &str,
        state: &'a CoordinatorState,
    ) -> Option<&'a ActorRef<PeerMessage>> {
        state.authenticated_peers()
            .find(|(_, entry)| entry.auth.as_ref().map(|a| a.did.as_str()) == Some(user_did))
            .map(|(_, entry)| &entry.actor)
    }
}
