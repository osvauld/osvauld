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
//!
//! # Generic Transport
//!
//! The Coordinator is generic over `C: Connection` to support different transports:
//! - `Coordinator<IrohConnection>`: Production with NAT traversal
//! - `Coordinator<MockConnection>`: Unit tests with in-memory channels

mod state;

use std::marker::PhantomData;
use std::sync::Arc;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::{debug, error, info, instrument, warn};
use transport::{Connection, NodeId};

use tokio::sync::{broadcast, mpsc, oneshot};

use crate::handle::CourierEvent;
use crate::peer_actor::{PeerActor, PeerActorArgs, PeerMessage};
use crate::state::PeerType;
use crate::trace::MessageTrace;
use butler::Butler;

pub use state::{CoordinatorState, PeerEntry, PeerInfo};

// Type aliases for production use
pub type IrohCoordinator = Coordinator<transport::IrohConnection>;
pub type IrohCoordinatorMessage = CoordinatorMessage<transport::IrohConnection>;

/// Request for CourierRunner to initiate a connection
#[derive(Debug, Clone)]
pub struct ConnectRequest {
    pub node_id: NodeId,
    pub permit: String,
}

/// Messages received by Coordinator
///
/// Generic over `C: Connection` to support different transport implementations.
#[derive(Debug)]
pub enum CoordinatorMessage<C: Connection> {
    /// New connection from transport
    Connected { node_id: NodeId, conn: C },

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

    /// Connect to peer (fire-and-forget, result via CourierEvent)
    ///
    /// **Context**: App wants to connect to a peer
    /// **We do**: Store permit, mark as pending, request transport connection
    /// **Events emitted**: PeerAuthenticated on success, ConnectionFailed on failure
    Connect { node_id: NodeId, permit: String },

    /// Get PeerActor reference for direct communication
    GetPeerActor {
        node_id: NodeId,
        response: oneshot::Sender<Option<ActorRef<PeerMessage>>>,
    },

    /// Relay datagram to other peers (Node mode only)
    RelayDatagram { from_node_id: NodeId, data: Vec<u8> },

    /// Broadcast datagram to all authenticated peers
    BroadcastDatagram { data: Vec<u8> },

    /// Scribe requested sync with a user
    EnsureSync { user_did: String },

    /// Subscribe to dynamic layers on a creator peer
    ///
    /// **Context**: Node's Scribe detected new entries in creator's __sync_meta.
    /// Route to PeerActor connected to creator_did, send LayerSubscribe for each layer.
    SubscribeLayers {
        page_id: String,
        creator_did: String,
        layers: Vec<String>,
    },

    /// Distribute updated page permits to peers (Node mode)
    ///
    /// **Context**: New app installed → page permits reissued with new layers
    /// **We do**: Store updated permits in butler, send PermitUpdate to each recipient's PeerActor
    DistributePagePermitUpdates {
        page_id: String,
        permits: Vec<(String, String)>, // (recipient_did, new_permit_token)
    },

    /// Page opened - trigger subscription refresh for all authenticated peers
    ///
    /// **Context**: App opened a page, we need to ensure subscriptions are set up
    /// **We do**: Send RefreshSubscriptions to all authenticated PeerActors
    PageOpened { page_id: String },

    /// Check if a node is authenticated
    ///
    /// **Context**: Script/app wants to wait for auth before publishing
    /// **Returns**: true if handshake complete, false otherwise
    IsNodeAuthenticated {
        node_id: NodeId,
        response: oneshot::Sender<bool>,
    },

    /// Viewer received space metadata from node (PeerActor -> Coordinator)
    ///
    /// **Context**: Viewer's PeerActor received SpaceData, stored space + page shells
    /// **We emit**: ViewerSpaceReceived event to app
    ViewerSpaceReceived {
        node_id: NodeId,
        space: domains::Space,
        page_count: usize,
    },

    /// Viewer completed initial permit sync (PeerActor -> Coordinator)
    ///
    /// **Context**: Viewer received all expected page permits via PermitUpdate
    /// **We emit**: ViewerSyncComplete event to app
    ViewerSyncComplete {
        node_id: NodeId,
        space_id: String,
        pages_synced: usize,
    },

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
///
/// Generic over `C: Connection` to support different transport implementations.
pub struct Coordinator<C: Connection> {
    _phantom: PhantomData<C>,
}

impl<C: Connection> Coordinator<C> {
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Emit an event to the app layer via event_tx channel
    fn emit_event(state: &CoordinatorState<C>, event: CourierEvent) {
        // Serialize to capture broadcast if active
        if let Some(tx) = &state.capture_tx {
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "courier_event",
                "ts": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                "data": &event,
            })) {
                let _ = tx.send(json);
            }
        }

        if let Some(tx) = &state.event_tx {
            if let Err(e) = tx.try_send(event) {
                warn!("Failed to emit event: {}", e);
            }
        }
    }

    /// Process a ConnectionEvent and convert to CoordinatorMessage
    ///
    /// **Note**: Transport now only emits lifecycle events (Connected/Disconnected).
    /// Message reading is done by PeerSession directly from the Connection.
    pub fn from_connection_event(
        event: transport::ConnectionEvent<C>,
    ) -> Option<CoordinatorMessage<C>> {
        match event {
            transport::ConnectionEvent::Connected { node_id, conn } => {
                Some(CoordinatorMessage::Connected { node_id, conn })
            }
            transport::ConnectionEvent::Disconnected { node_id } => {
                Some(CoordinatorMessage::Disconnected { node_id })
            }
        }
    }
}

impl<C: Connection> Default for Coordinator<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Connection> Actor for Coordinator<C> {
    type Msg = CoordinatorMessage<C>;
    type State = CoordinatorState<C>;
    type Arguments = (
        NodeId,
        CourierMode,
        Arc<Butler>,
        crate::peer_actor::BlobStore,
        Option<mpsc::Sender<ConnectRequest>>,
        Option<mpsc::Sender<CourierEvent>>,
        Option<mpsc::UnboundedSender<MessageTrace>>,
        Option<broadcast::Sender<String>>,
    );

    #[instrument(skip_all)]
    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (our_node_id, mode, butler, blob_store, connect_tx, event_tx, message_tx, capture_tx) =
            args;

        info!(
            "Coordinator started in {:?} mode (node_id={})",
            mode, our_node_id
        );

        Ok(CoordinatorState::new(
            our_node_id,
            mode,
            butler,
            blob_store,
            connect_tx,
            event_tx,
            message_tx,
            capture_tx,
        ))
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            CoordinatorMessage::Connected { node_id, conn } => {
                self.on_connected(myself.clone(), node_id, conn, state)
                    .await;
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
                info!(
                    "Peer authenticated: {} ({}) - {:?}",
                    username, node_id, peer_type
                );
                state.authenticate_peer(node_id, peer_type, did.clone(), username.clone());

                // Mark sovereign node as connected (for User mode connecting to their Node)
                if peer_type == PeerType::MyNode {
                    let node_id_str = node_id.to_string();
                    match state.butler.nodes().set_connected(&node_id_str, true) {
                        Ok(true) => info!("Marked sovereign node {} as connected", node_id_str),
                        Ok(false) => debug!("Sovereign node {} not found in storage", node_id_str),
                        Err(e) => warn!("Failed to mark sovereign node connected: {}", e),
                    }
                }

                // Emit event for app layer
                Self::emit_event(
                    state,
                    CourierEvent::PeerAuthenticated {
                        node_id: node_id.to_string(),
                        did,
                        username,
                    },
                );
            }

            CoordinatorMessage::PeerFailed { node_id, reason } => {
                warn!("Peer failed: {} - {}", node_id, reason);

                // Emit failure event for app layer
                Self::emit_event(
                    state,
                    CourierEvent::ConnectionFailed {
                        node_id: node_id.to_string(),
                        error: reason,
                    },
                );

                state.remove_peer(node_id);
            }

            CoordinatorMessage::Connect { node_id, permit } => {
                self.on_connect(node_id, permit, state).await;
            }

            CoordinatorMessage::GetPeerActor { node_id, response } => {
                let actor = state.get_actor(&node_id).cloned();
                let _ = response.send(actor);
            }

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

            CoordinatorMessage::EnsureSync { user_did } => {
                self.on_ensure_sync(myself.clone(), user_did, state).await;
            }

            CoordinatorMessage::SubscribeLayers {
                page_id,
                creator_did,
                layers,
            } => {
                self.on_subscribe_layers(&page_id, &creator_did, layers, state)
                    .await;
            }

            CoordinatorMessage::DistributePagePermitUpdates { page_id, permits } => {
                self.on_distribute_page_permit_updates(&page_id, &permits, state)
                    .await;
            }

            CoordinatorMessage::PageOpened { page_id } => {
                self.on_page_opened(&page_id, state).await;
            }

            CoordinatorMessage::IsNodeAuthenticated { node_id, response } => {
                let is_auth = state.is_authenticated(&node_id);
                let _ = response.send(is_auth);
            }

            CoordinatorMessage::ViewerSpaceReceived {
                node_id,
                space,
                page_count,
            } => {
                info!(
                    "Viewer received space {} from {} ({} pages)",
                    space.id, node_id, page_count
                );
                Self::emit_event(
                    state,
                    CourierEvent::ViewerSpaceReceived {
                        node_id: node_id.to_string(),
                        space,
                        page_count,
                    },
                );
            }

            CoordinatorMessage::ViewerSyncComplete {
                node_id,
                space_id,
                pages_synced,
            } => {
                info!(
                    "Viewer sync complete for space {} from {} ({} pages)",
                    space_id, node_id, pages_synced
                );
                Self::emit_event(
                    state,
                    CourierEvent::ViewerSyncComplete {
                        node_id: node_id.to_string(),
                        space_id,
                        pages_synced,
                    },
                );
            }

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
                debug!("Child actor {} terminated: {:?}", cell.get_id(), reason);
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

impl<C: Connection> Coordinator<C> {
    /// Handle new connection - spawn PeerActor with permit if outbound
    #[instrument(skip_all, fields(node_id = %node_id))]
    async fn on_connected(
        &self,
        myself: ActorRef<CoordinatorMessage<C>>,
        node_id: NodeId,
        conn: C,
        state: &mut CoordinatorState<C>,
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
            warn!(
                "Outbound connection to {} but no permit - cannot handshake",
                node_id
            );
        }

        // Spawn PeerActor - if permit provided, it auto-initiates handshake
        let peer_actor = PeerActor::<C>::new(node_id);
        let args = PeerActorArgs {
            mode: state.mode,
            conn: conn.clone(),
            coordinator: myself.clone(),
            butler: state.butler.clone(),
            blob_store: state.blob_store.clone(),
            permit,
            message_tx: state.message_tx.clone(),
            our_node_id: state.our_node_id,
            capture_tx: state.capture_tx.clone(),
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
    #[instrument(skip_all, fields(node_id = %node_id))]
    async fn on_disconnected(&self, node_id: NodeId, state: &mut CoordinatorState<C>) {
        info!("Disconnected: {}", node_id);

        if let Some(entry) = state.peers.remove(&node_id) {
            entry.actor.stop(Some("disconnected".to_string()));
        }

        state.pending_connections.remove(&node_id);
        state.pending_permits.remove(&node_id);

        // Emit disconnect event for app layer
        Self::emit_event(
            state,
            CourierEvent::PeerDisconnected {
                node_id: node_id.to_string(),
            },
        );
    }

    /// Connect to peer (fire-and-forget, result via events)
    ///
    /// **Context**: App calls connect(), we store permit and mark as pending
    /// **Transport**: CourierRunner sees pending_connections and calls transport.connect()
    /// **Events**: PeerAuthenticated on success, ConnectionFailed on failure
    #[instrument(skip_all, fields(node_id = %node_id))]
    async fn on_connect(&self, node_id: NodeId, permit: String, state: &mut CoordinatorState<C>) {
        info!("Connect: node={}", node_id);

        // Check if already authenticated - emit event immediately
        if state.is_authenticated(&node_id) {
            info!("Peer {} already authenticated", node_id);
            if let Some(auth) = state.get_auth(&node_id) {
                Self::emit_event(
                    state,
                    CourierEvent::PeerAuthenticated {
                        node_id: node_id.to_string(),
                        did: auth.did.clone(),
                        username: auth.username.clone(),
                    },
                );
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
                Self::emit_event(
                    state,
                    CourierEvent::ConnectionFailed {
                        node_id: node_id.to_string(),
                        error: format!("Failed to initiate connection: {}", e),
                    },
                );
            }
        }
    }

    /// Handle EnsureSync from Scribe - connect to user if not already connected
    #[instrument(skip_all, fields(user_did = %user_did))]
    async fn on_ensure_sync(
        &self,
        _myself: ActorRef<CoordinatorMessage<C>>,
        user_did: String,
        state: &mut CoordinatorState<C>,
    ) {
        info!(user_did = %user_did, "Scribe requested sync with user");

        // Check if user is already connected
        if let Some(peer_actor) = self.get_peer_actor_for_user(&user_did, state) {
            info!(user_did = %user_did, "User already connected, sending RefreshSubscriptions to PeerActor");
            let _ = peer_actor.cast(PeerMessage::RefreshSubscriptions);
            return;
        }

        // Resolve user_did to device info
        let device_info = match state.butler.contacts().resolve_device(&user_did) {
            Ok(Some(info)) => info,
            Ok(None) => {
                // In Node mode, viewers initiate connections - we don't reconnect to them
                // This is expected behavior, not a warning condition
                if state.mode == CourierMode::Node {
                    debug!(user_did = %user_did, "Viewer not connected (will reconnect when online)");
                } else {
                    warn!(user_did = %user_did, "No device info found for user");
                }
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
        state
            .pending_permits
            .insert(node_id, device_info.permit.clone());

        if let Some(ref tx) = state.connect_tx {
            if let Err(e) = tx.try_send(ConnectRequest {
                node_id,
                permit: device_info.permit.clone(),
            }) {
                warn!(node_id = %node_id, error = %e, "Failed to send connect request");
            }
        }

        Self::emit_event(
            state,
            CourierEvent::ConnectRequested {
                node_id: node_id.to_string(),
                permit: device_info.permit,
            },
        );
    }

    /// Handle SubscribeLayers - send LayerSubscribe to creator's PeerActor
    ///
    /// **Context**: Node's Scribe detected new dynamic layers in creator's __sync_meta.
    /// Route to PeerActor connected to creator_did, which sends LayerSubscribe for each layer.
    #[instrument(skip_all, fields(page_id = %page_id, creator_did = %creator_did, layer_count = layers.len()))]
    async fn on_subscribe_layers(
        &self,
        page_id: &str,
        creator_did: &str,
        layers: Vec<String>,
        state: &mut CoordinatorState<C>,
    ) {
        info!(page_id = %page_id, creator_did = %creator_did, "SubscribeLayers: looking for peer actor");

        if let Some(peer_actor) = self.get_peer_actor_for_user(creator_did, state) {
            // PeerActor handles consent issuance + LayerSubscribe sending
            let _ = peer_actor.cast(PeerMessage::SubscribeLayers {
                page_id: page_id.to_string(),
                layers: layers.clone(),
            });
            info!(
                page_id = %page_id,
                creator_did = %creator_did,
                count = layers.len(),
                "Sent LayerSubscribe messages to creator's PeerActor"
            );
        } else {
            warn!(creator_did = %creator_did, "SubscribeLayers: creator not connected");
        }
    }

    /// Find peer actor for a user by DID
    fn get_peer_actor_for_user<'a>(
        &self,
        user_did: &str,
        state: &'a CoordinatorState<C>,
    ) -> Option<&'a ActorRef<PeerMessage>> {
        state
            .authenticated_peers()
            .find(|(_, entry)| entry.auth.as_ref().map(|a| a.did.as_str()) == Some(user_did))
            .map(|(_, entry)| &entry.actor)
    }

    /// Distribute updated page permits after reissue (Node mode)
    ///
    /// **Context**: New app installed → page permits reissued with new layers
    /// **We do**: Store updated permits in butler, send PermitUpdate to each recipient
    #[instrument(skip_all, fields(page_id = %page_id, permit_count = permits.len()))]
    async fn on_distribute_page_permit_updates(
        &self,
        page_id: &str,
        permits: &[(String, String)],
        state: &mut CoordinatorState<C>,
    ) {
        info!(
            "Distributing {} page permit updates for page={}",
            permits.len(),
            page_id
        );

        for (recipient_did, permit_token) in permits {
            // Store updated permit in butler
            if let Err(e) =
                state
                    .butler
                    .permits()
                    .page()
                    .store(page_id, recipient_did, permit_token)
            {
                warn!(
                    recipient = %recipient_did,
                    error = %e,
                    "Failed to store updated page permit"
                );
                continue;
            }

            // Find PeerActor for this recipient and send
            if let Some(actor) = state.get_peer_actor_for_did(recipient_did) {
                if let Err(e) = actor.cast(PeerMessage::SendPermitUpdate {
                    page_id: page_id.to_string(),
                    permit: permit_token.clone(),
                }) {
                    warn!(
                        recipient = %recipient_did,
                        error = %e,
                        "Failed to send PermitUpdate to PeerActor"
                    );
                } else {
                    info!(recipient = %recipient_did, "Sent PermitUpdate to PeerActor");
                }
            } else {
                // Peer not connected — permit stored, will be picked up on reconnect
                debug!(
                    recipient = %recipient_did,
                    "Peer not connected, updated page permit stored for reconnect"
                );
            }
        }
    }

    /// Handle page opened - refresh subscriptions for all authenticated peers
    ///
    /// **Context**: App opened a page, subscriptions may not be established yet
    /// **We do**: Send RefreshSubscriptions to all authenticated PeerActors
    /// **Design**: Ensures subscription after page open (race condition fix)
    #[instrument(skip_all, fields(page_id = %page_id))]
    async fn on_page_opened(&self, page_id: &str, state: &mut CoordinatorState<C>) {
        let peer_count = state.authenticated_peers().count();
        if peer_count == 0 {
            debug!(page_id = %page_id, "No authenticated peers to refresh subscriptions for");
            return;
        }

        info!(page_id = %page_id, peer_count = peer_count, "Page opened, refreshing subscriptions for authenticated peers");

        for (node_id, entry) in state.authenticated_peers() {
            if let Err(e) = entry.actor.cast(PeerMessage::RefreshSubscriptions) {
                warn!(node_id = %node_id, error = %e, "Failed to send RefreshSubscriptions");
            }
        }
    }
}
