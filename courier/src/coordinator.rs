//! Coordinator - Supervisor and message router
//!
//! The Coordinator is the top-level actor that:
//! - Receives bytes from transport, deserializes to Message
//! - Routes messages to PeerActors
//! - Spawns PeerActors on new connections
//! - Supervises child actors (restart policy)

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use ractor::{Actor, ActorProcessingErr, ActorRef, SupervisionEvent};
use tracing::{debug, error, info, warn};
use transport::{ConnectionHandle, NodeId, TransportEvent};

use tokio::sync::{mpsc, oneshot};

use crate::handle::CourierEvent;
use crate::message::Message;
use crate::peer_actor::{PeerActor, PeerActorArgs, PeerMessage};
use crate::state::PeerType;
use butler::Butler;

/// Messages received by Coordinator
#[derive(Debug)]
pub enum CoordinatorMessage {
    /// Raw bytes from transport - will be deserialized
    TransportBytes { node_id: NodeId, data: Vec<u8> },

    /// New connection from transport
    Connected {
        node_id: NodeId,
        conn: ConnectionHandle,
    },

    /// Connection disconnected
    Disconnected { node_id: NodeId },

    /// Initiate handshake with a peer (User mode only)
    ///
    /// Sends Hello message with the provided permit to start authentication.
    InitiateHandshake {
        node_id: NodeId,
        permit: String,
    },

    /// Mark that we're about to initiate an outbound connection
    ///
    /// Used to track that we initiated the connection (so we send Hello first).
    OutboundConnection { node_id: NodeId },

    /// PeerActor completed handshake
    PeerAuthenticated {
        node_id: NodeId,
        peer_type: PeerType,
        did: String,
        username: String,
    },

    /// PeerActor failed or disconnected
    PeerFailed { node_id: NodeId, reason: String },

    // ==================== Sync Events ====================

    /// Scribe requested sync with a user (EnsureSync from Scribe)
    ///
    /// **Context**: Scribe has an update for a user not currently subscribed.
    /// **We do**: Resolve user_did → device, connect if needed, PeerActor subscribes.
    EnsureSync { user_did: String },

    // ==================== Publishing Commands ====================

    /// Publish a space to a connected node (User mode)
    PublishSpace {
        node_id: NodeId,
        space_id: String,
    },

    /// Space was successfully published (from PeerActor)
    SpacePublished {
        node_id: NodeId,
        space_id: String,
        /// Page IDs already on the node
        existing_pages: Vec<String>,
    },

    /// Publish failed (from PeerActor)
    PublishFailed {
        node_id: NodeId,
        request_id: String,
        error: String,
    },

    /// Publish a page to a connected node (User mode)
    PublishPage {
        node_id: NodeId,
        page_id: String,
    },

    // ==================== Shareable Links ====================

    /// Request shareable link for a space (User mode → Node)
    ///
    /// Owner requests node to generate a viewer permit with aud:* (wildcard audience).
    GetShareableLink {
        node_id: NodeId,
        space_id: String,
    },

    /// Shareable link response (from PeerActor)
    ShareableLinkReceived {
        node_id: NodeId,
        space_id: String,
        /// Viewer permit with aud:* (one-time shareable token)
        permit: String,
    },

    /// Page was successfully published (from PeerActor)
    PagePublished {
        node_id: NodeId,
        page_id: String,
    },

    /// Viewer sync completed (from PeerActor)
    ///
    /// **Context**: Viewer received and stored space and pages from node
    ViewerSyncComplete {
        node_id: NodeId,
        space_id: String,
        pages_synced: usize,
    },

    /// Sync consent handshake completed (from PeerActor)
    ///
    /// **Context**: Viewer received SyncConsentAck from node, consent permits stored
    SyncConsentComplete {
        node_id: NodeId,
        space_id: String,
    },

    /// Viewer received space from node (emit to app for UI updates)
    ViewerSpaceReceived {
        node_id: NodeId,
        space: butler::Space,
        page_count: usize,
    },

    /// Viewer received a page from node (emit to app for UI updates)
    ViewerPageReceived {
        node_id: NodeId,
        page: butler::Page,
        is_last: bool,
    },

    /// Request space content as viewer (User mode → Node)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link
    /// **We send**: SpaceRequest to node with permit
    RequestSpaceAsViewer {
        node_id: NodeId,
        space_id: String,
        /// The aud:* permit from shareable link
        viewer_permit: String,
    },

    // ==================== Live Sync ====================

    /// Subscribe to live updates for a page from a specific peer
    ///
    /// **Context**: After handshake, app wants to receive live updates for a page
    /// **We do**: Route to PeerActor with permit from storage
    SubscribeToPage {
        node_id: NodeId,
        page_id: String,
    },

    /// Connect to peer and notify when authenticated
    ///
    /// **Context**: Caller wants to connect and wait for handshake to complete
    /// **We do**: Initiate connection, register waiter, notify when PeerAuthenticated
    ConnectAndAuth {
        node_id: NodeId,
        permit: String,
        response: oneshot::Sender<Result<NodeId, String>>,
    },

    /// Shutdown all actors
    Shutdown,
}

/// Info about an authenticated peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub peer_type: PeerType,
    pub did: String,
    pub username: String,
}

/// Courier mode (User or Node)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourierMode {
    /// Running as user's device (connects TO node)
    User,
    /// Running as always-on node (accepts connections)
    Node,
}

/// Actor state for Coordinator
pub struct CoordinatorState {
    /// Our node ID (for unique actor naming)
    our_node_id: NodeId,
    /// Mode of operation (User or Node)
    mode: CourierMode,
    /// Butler for storage and identity
    butler: Arc<Butler>,
    /// Registry of PeerActors by NodeId
    peer_actors: HashMap<NodeId, ActorRef<PeerMessage>>,
    /// Connection handles for sending responses
    connections: HashMap<NodeId, ConnectionHandle>,
    /// Authenticated peer info
    authenticated_peers: HashMap<NodeId, PeerInfo>,
    /// Outbound connections we initiated (to know when to send Hello first)
    pending_connections: HashSet<NodeId>,
    /// Channels waiting for peer authentication completion
    /// Used by connect_and_wait_for_auth to notify callers when handshake completes
    auth_waiters: HashMap<NodeId, Vec<oneshot::Sender<Result<NodeId, String>>>>,
    /// Permits for pending outbound connections
    /// Used by connect_and_wait_for_auth to pass permit to on_connected
    pending_permits: HashMap<NodeId, String>,
    /// Channel to emit events to app layer (protocol-agnostic)
    event_tx: Option<mpsc::Sender<CourierEvent>>,
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
    pub fn from_transport_event(event: TransportEvent) -> Option<CoordinatorMessage> {
        match event {
            TransportEvent::Connected { node_id, conn } => {
                Some(CoordinatorMessage::Connected { node_id, conn })
            }
            TransportEvent::Disconnected { node_id } => {
                Some(CoordinatorMessage::Disconnected { node_id })
            }
            TransportEvent::Bytes { node_id, data } => {
                Some(CoordinatorMessage::TransportBytes { node_id, data })
            }
            TransportEvent::Error { node_id, error } => {
                if let Some(node_id) = node_id {
                    Some(CoordinatorMessage::PeerFailed {
                        node_id,
                        reason: error,
                    })
                } else {
                    warn!("Transport error without node_id: {}", error);
                    None
                }
            }
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
    type Arguments = (NodeId, CourierMode, Arc<Butler>, Option<mpsc::Sender<CourierEvent>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (our_node_id, mode, butler, event_tx) = args;

        info!("Coordinator started in {:?} mode (node_id={})", mode, our_node_id);

        Ok(CoordinatorState {
            our_node_id,
            mode,
            butler,
            peer_actors: HashMap::new(),
            connections: HashMap::new(),
            authenticated_peers: HashMap::new(),
            pending_connections: HashSet::new(),
            auth_waiters: HashMap::new(),
            pending_permits: HashMap::new(),
            event_tx,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            CoordinatorMessage::TransportBytes { node_id, data } => {
                self.on_bytes(node_id, data, state).await;
            }

            CoordinatorMessage::Connected { node_id, conn } => {
                self.on_connected(myself.clone(), node_id, conn, state)
                    .await;
            }

            CoordinatorMessage::Disconnected { node_id } => {
                self.on_disconnected(node_id, state).await;
            }

            CoordinatorMessage::InitiateHandshake { node_id, permit } => {
                self.on_initiate_handshake(node_id, permit, state).await;
            }

            CoordinatorMessage::OutboundConnection { node_id } => {
                debug!("Marking outbound connection to {}", node_id);
                state.pending_connections.insert(node_id);
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
                state.authenticated_peers.insert(
                    node_id,
                    PeerInfo {
                        node_id,
                        peer_type,
                        did: did.clone(),
                        username: username.clone(),
                    },
                );

                // Mark sovereign node as connected (for User mode connecting to their Node)
                // This enables sync_target resolution in Scribe
                if peer_type == PeerType::MyNode {
                    let node_id_str = node_id.to_string();
                    match state.butler.set_sovereign_node_connected(&node_id_str, true) {
                        Ok(true) => info!("Marked sovereign node {} as connected", node_id_str),
                        Ok(false) => debug!("Sovereign node {} not found in storage", node_id_str),
                        Err(e) => warn!("Failed to mark sovereign node connected: {}", e),
                    }
                }

                // Notify any waiters that auth completed
                if let Some(waiters) = state.auth_waiters.remove(&node_id) {
                    info!("Notifying {} auth waiters for {}", waiters.len(), node_id);
                    for waiter in waiters {
                        let _ = waiter.send(Ok(node_id));
                    }
                }
            }

            CoordinatorMessage::PeerFailed { node_id, reason } => {
                warn!("Peer failed: {} - {}", node_id, reason);
                state.authenticated_peers.remove(&node_id);
                state.peer_actors.remove(&node_id);
                state.connections.remove(&node_id);

                // Notify any waiters that auth failed
                if let Some(waiters) = state.auth_waiters.remove(&node_id) {
                    info!("Notifying {} auth waiters of failure for {}", waiters.len(), node_id);
                    for waiter in waiters {
                        let _ = waiter.send(Err(reason.clone()));
                    }
                }
            }

            CoordinatorMessage::PublishSpace { node_id, space_id } => {
                self.on_publish_space(node_id, space_id, state).await;
            }

            CoordinatorMessage::SpacePublished { node_id, space_id, existing_pages } => {
                info!(
                    "Space {} published to node {} ({} existing pages)",
                    space_id, node_id, existing_pages.len()
                );
                // Trigger page sync: publish all pages not already on node
                self.on_space_published(myself.clone(), node_id, &space_id, &existing_pages, state).await;
            }

            CoordinatorMessage::PublishFailed { node_id, request_id, error } => {
                error!(
                    "Publish failed on node {}: {} (request: {})",
                    node_id, error, request_id
                );
            }

            CoordinatorMessage::PublishPage { node_id, page_id } => {
                self.on_publish_page(node_id, page_id, state).await;
            }

            CoordinatorMessage::PagePublished { node_id, page_id } => {
                info!("Page {} published to node {}", page_id, node_id);
            }

            CoordinatorMessage::GetShareableLink { node_id, space_id } => {
                self.on_get_shareable_link(node_id, space_id, state).await;
            }

            CoordinatorMessage::ShareableLinkReceived { node_id, space_id, permit } => {
                info!(
                    "Shareable link received for space {} from node {} (permit len: {})",
                    space_id, node_id, permit.len()
                );
                Self::emit_event(state, CourierEvent::ShareableLinkReceived {
                    node_id: node_id.to_string(),
                    space_id,
                    permit,
                });
            }

            CoordinatorMessage::ViewerSyncComplete { node_id, space_id, pages_synced } => {
                info!(
                    "Viewer sync complete for space {} from node {} ({} pages)",
                    space_id, node_id, pages_synced
                );
                Self::emit_event(state, CourierEvent::ViewerSyncComplete {
                    node_id: node_id.to_string(),
                    space_id,
                    pages_synced,
                });
            }

            CoordinatorMessage::SyncConsentComplete { node_id, space_id } => {
                info!(
                    "Sync consent complete for space {} from node {}",
                    space_id, node_id
                );
                Self::emit_event(state, CourierEvent::SyncConsentComplete {
                    node_id: node_id.to_string(),
                    space_id,
                });
            }

            CoordinatorMessage::ViewerSpaceReceived { node_id, space, page_count } => {
                info!(
                    "Viewer received space {} ({}) from node {} ({} pages expected)",
                    space.id, space.name, node_id, page_count
                );
                Self::emit_event(state, CourierEvent::ViewerSpaceReceived {
                    node_id: node_id.to_string(),
                    space,
                    page_count,
                });
            }

            CoordinatorMessage::ViewerPageReceived { node_id, page, is_last } => {
                info!(
                    "Viewer received page {} ({}) from node {} (is_last: {})",
                    page.id, page.name, node_id, is_last
                );
                Self::emit_event(state, CourierEvent::ViewerPageReceived {
                    node_id: node_id.to_string(),
                    page,
                    is_last,
                });
            }

            CoordinatorMessage::RequestSpaceAsViewer { node_id, space_id, viewer_permit } => {
                self.on_request_space_as_viewer(node_id, space_id, viewer_permit, state).await;
            }

            CoordinatorMessage::SubscribeToPage { node_id, page_id } => {
                self.on_subscribe_to_page(node_id, page_id, state).await;
            }

            CoordinatorMessage::ConnectAndAuth { node_id, permit, response } => {
                self.on_connect_and_auth(node_id, permit, response, state).await;
            }

            CoordinatorMessage::EnsureSync { user_did } => {
                self.on_ensure_sync(myself.clone(), user_did, state).await;
            }

            CoordinatorMessage::Shutdown => {
                info!("Coordinator shutting down");
                // Stop all peer actors
                for (node_id, actor) in state.peer_actors.drain() {
                    debug!("Stopping PeerActor {}", node_id);
                    actor.stop(Some("coordinator shutdown".to_string()));
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
                // Find and remove from registry
                state.peer_actors.retain(|node_id, actor| {
                    if actor.get_id() == cell.get_id() {
                        debug!("Removing terminated PeerActor {}", node_id);
                        state.authenticated_peers.remove(node_id);
                        state.connections.remove(node_id);
                        false
                    } else {
                        true
                    }
                });
            }
            SupervisionEvent::ActorFailed(cell, error) => {
                error!("Child actor {} failed: {}", cell.get_id(), error);
                // Same cleanup as terminated
                state.peer_actors.retain(|node_id, actor| {
                    if actor.get_id() == cell.get_id() {
                        debug!("Removing failed PeerActor {}", node_id);
                        state.authenticated_peers.remove(node_id);
                        state.connections.remove(node_id);
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
    /// Handle raw bytes from transport
    async fn on_bytes(&self, node_id: NodeId, data: Vec<u8>, state: &mut CoordinatorState) {
        // Deserialize message
        let message = match Message::from_bytes(&data) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to deserialize message from {}: {}", node_id, e);
                return;
            }
        };

        debug!("Received {:?} from {}", message, node_id);

        // Route to PeerActor
        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::Protocol(message));
        } else {
            warn!("No PeerActor for message from {}", node_id);
        }
    }

    /// Handle new connection
    async fn on_connected(
        &self,
        myself: ActorRef<CoordinatorMessage>,
        node_id: NodeId,
        conn: ConnectionHandle,
        state: &mut CoordinatorState,
    ) {
        info!("New connection from: {}", node_id);

        // Store connection handle
        state.connections.insert(node_id, conn.clone());

        // Spawn PeerActor
        let peer_actor = PeerActor::new(node_id);
        let args = PeerActorArgs {
            mode: state.mode,
            conn,
            coordinator: myself.clone(),
            butler: state.butler.clone(),
        };

        match Actor::spawn_linked(
            Some(format!("peer-{}-{}", state.our_node_id, node_id)),
            peer_actor,
            args,
            myself.get_cell(),
        )
        .await
        {
            Ok((actor_ref, _)) => {
                state.peer_actors.insert(node_id, actor_ref.clone());
                debug!("PeerActor spawned for {}", node_id);

                // Check if this was an outbound connection WE initiated
                if state.pending_connections.remove(&node_id) {
                    // Get permit from pending_permits (set by connect_and_wait_for_auth)
                    if let Some(permit) = state.pending_permits.remove(&node_id) {
                        let _ = actor_ref.cast(PeerMessage::InitiateHandshake { permit });
                        info!("Initiated handshake with {}", node_id);
                    } else {
                        warn!("No pending permit for {} - cannot handshake", node_id);
                    }
                }
                // else: inbound connection - wait for their Hello
            }
            Err(e) => {
                error!("Failed to spawn PeerActor for {}: {:?}", node_id, e);
                // Clean up pending connection if spawn failed
                state.pending_connections.remove(&node_id);
            }
        }
    }

    /// Handle disconnection
    async fn on_disconnected(&self, node_id: NodeId, state: &mut CoordinatorState) {
        info!("Disconnected: {}", node_id);

        // Stop the PeerActor
        if let Some(actor) = state.peer_actors.remove(&node_id) {
            actor.stop(Some("disconnected".to_string()));
        }

        // Remove from registries
        state.authenticated_peers.remove(&node_id);
        state.connections.remove(&node_id);

        // Notify any waiters that connection was lost
        if let Some(waiters) = state.auth_waiters.remove(&node_id) {
            for waiter in waiters {
                let _ = waiter.send(Err("Disconnected".to_string()));
            }
        }
    }

    /// Initiate handshake with a peer (User mode only)
    ///
    /// Forwards the permit to the PeerActor to send Hello.
    async fn on_initiate_handshake(
        &self,
        node_id: NodeId,
        permit: String,
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("InitiateHandshake called in Node mode - ignoring");
            return;
        }

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::InitiateHandshake { permit });
            debug!("Sent InitiateHandshake to PeerActor for {}", node_id);
        } else {
            warn!("No PeerActor for {} - cannot initiate handshake", node_id);
        }
    }

    /// Publish a space to a connected node (User mode only)
    ///
    /// Forwards the request to the PeerActor which will prepare and send PublishSpace.
    async fn on_publish_space(
        &self,
        node_id: NodeId,
        space_id: String,
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("PublishSpace called in Node mode - ignoring");
            return;
        }

        // Verify peer is authenticated
        if !state.authenticated_peers.contains_key(&node_id) {
            warn!("Cannot publish to unauthenticated peer: {}", node_id);
            return;
        }

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::PublishSpace { space_id: space_id.clone() });
            info!("Sent PublishSpace command to PeerActor for {}", node_id);
        } else {
            warn!("No PeerActor for {} - cannot publish space", node_id);
        }
    }

    /// Handle PublishPage command (User mode only)
    ///
    /// Forwards the command to the appropriate PeerActor.
    async fn on_publish_page(
        &self,
        node_id: NodeId,
        page_id: String,
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("PublishPage called in Node mode - ignoring");
            return;
        }

        // Verify peer is authenticated
        if !state.authenticated_peers.contains_key(&node_id) {
            warn!("Cannot publish to unauthenticated peer: {}", node_id);
            return;
        }

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::PublishPage { page_id: page_id.clone() });
            info!("Sent PublishPage command to PeerActor for {}", node_id);
        } else {
            warn!("No PeerActor for {} - cannot publish page", node_id);
        }
    }

    /// Handle SpacePublished - trigger page sync
    ///
    /// **Context**: Space was published, now publish all pages in the space
    /// **We do**: Get all pages in space, filter existing, send PublishPage for each
    async fn on_space_published(
        &self,
        myself: ActorRef<CoordinatorMessage>,
        node_id: NodeId,
        space_id: &str,
        existing_pages: &[String],
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("on_space_published called in Node mode - ignoring");
            return;
        }

        // Get all pages in space from Butler
        let pages = match state.butler.list_page_ids_for_space(space_id) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to list pages for space {}: {}", space_id, e);
                return;
            }
        };

        // Filter out pages already on node
        let pages_to_publish: Vec<String> = pages
            .into_iter()
            .filter(|p| !existing_pages.contains(p))
            .collect();

        if pages_to_publish.is_empty() {
            info!("No new pages to publish for space {}", space_id);
            return;
        }

        info!(
            "Publishing {} pages for space {} to node {}",
            pages_to_publish.len(), space_id, node_id
        );

        // Send PublishPage for each page
        for page_id in pages_to_publish {
            let _ = myself.cast(CoordinatorMessage::PublishPage {
                node_id,
                page_id,
            });
        }
    }

    /// Request shareable link from node (User mode only)
    ///
    /// **Context**: Owner wants to share a space with a viewer
    /// **We do**: Send GetShareableLink to PeerActor, which forwards to node
    async fn on_get_shareable_link(
        &self,
        node_id: NodeId,
        space_id: String,
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("GetShareableLink called in Node mode - ignoring");
            return;
        }

        // Verify peer is authenticated
        if !state.authenticated_peers.contains_key(&node_id) {
            warn!("Cannot request shareable link from unauthenticated peer: {}", node_id);
            return;
        }

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::GetShareableLink { space_id: space_id.clone() });
            info!("Sent GetShareableLink command to PeerActor for {}", node_id);
        } else {
            warn!("No PeerActor for {} - cannot request shareable link", node_id);
        }
    }

    /// Request space content as viewer (User mode only)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link
    /// **We do**: Send RequestSpaceAsViewer to PeerActor, which forwards SpaceRequest to node
    async fn on_request_space_as_viewer(
        &self,
        node_id: NodeId,
        space_id: String,
        viewer_permit: String,
        state: &mut CoordinatorState,
    ) {
        if state.mode != CourierMode::User {
            warn!("RequestSpaceAsViewer called in Node mode - ignoring");
            return;
        }

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::RequestSpaceAsViewer {
                space_id: space_id.clone(),
                viewer_permit,
            });
            info!("Sent RequestSpaceAsViewer command to PeerActor for {}", node_id);
        } else {
            warn!("No PeerActor for {} - cannot request space as viewer", node_id);
        }
    }

    /// Subscribe to live updates for a page
    ///
    /// **Context**: App wants to receive live sync updates for a page
    /// **We do**: Route to PeerActor with permit from storage
    async fn on_subscribe_to_page(
        &self,
        node_id: NodeId,
        page_id: String,
        state: &mut CoordinatorState,
    ) {
        // Verify peer is authenticated
        if !state.authenticated_peers.contains_key(&node_id) {
            warn!("Cannot subscribe via unauthenticated peer: {}", node_id);
            return;
        }

        // Get permit from storage
        let permit = match state.butler.get_page(&page_id) {
            Ok(Some(page)) => {
                match page.get_permit() {
                    Some(p) => p.clone(),
                    None => {
                        warn!("No permit found for page {} - cannot subscribe", page_id);
                        return;
                    }
                }
            }
            Ok(None) => {
                warn!("Page {} not found - cannot subscribe", page_id);
                return;
            }
            Err(e) => {
                error!("Failed to get page {}: {}", page_id, e);
                return;
            }
        };

        if let Some(peer_actor) = state.peer_actors.get(&node_id) {
            let _ = peer_actor.cast(PeerMessage::SubscribeToPage {
                page_id: page_id.clone(),
                permit,
            });
            info!("Sent SubscribeToPage command to PeerActor for {} (page: {})", node_id, page_id);
        } else {
            warn!("No PeerActor for {} - cannot subscribe to page", node_id);
        }
    }

    /// Handle ConnectAndAuth - connect to peer and wait for authentication
    ///
    /// **Context**: Caller wants to connect and receive notification when handshake completes
    /// **We do**: Register waiter, check if already authenticated, initiate connection if needed
    async fn on_connect_and_auth(
        &self,
        node_id: NodeId,
        permit: String,
        response: oneshot::Sender<Result<NodeId, String>>,
        state: &mut CoordinatorState,
    ) {
        info!("ConnectAndAuth: node={}", node_id);

        // Check if already authenticated - notify immediately
        if state.authenticated_peers.contains_key(&node_id) {
            info!("Peer {} already authenticated - notifying immediately", node_id);
            let _ = response.send(Ok(node_id));
            return;
        }

        // Register waiter
        state.auth_waiters
            .entry(node_id)
            .or_default()
            .push(response);

        // Check if already connected (handshake in progress)
        if state.peer_actors.contains_key(&node_id) {
            info!("Peer {} already connected - waiting for handshake to complete", node_id);
            return;
        }

        // Mark as pending outbound connection
        state.pending_connections.insert(node_id);

        // Store permit for on_connected to use when initiating handshake
        state.pending_permits.insert(node_id, permit);

        // Note: We don't emit ConnectRequested here because the caller
        // (CourierHandle::connect_and_wait_for_auth) already connects via transport.
        // ConnectRequested is only emitted by on_ensure_sync for Scribe-driven sync.
    }

    /// Handle EnsureSync event from Scribe.
    ///
    /// **Context**: Scribe has an update for a user not currently subscribed.
    /// **We do**:
    ///   1. Check if user is already connected (by any device)
    ///   2. If not, resolve user_did → device info
    ///   3. Initiate connection (PeerActor will subscribe on connect)
    async fn on_ensure_sync(
        &self,
        _myself: ActorRef<CoordinatorMessage>,
        user_did: String,
        state: &mut CoordinatorState,
    ) {
        info!(user_did = %user_did, "Scribe requested sync with user");

        // 1. Check if user is already connected
        if let Some(peer_actor) = self.get_peer_actor_for_user(&user_did, state) {
            // Peer is connected but may not be subscribed to new Scribes
            // Tell PeerActor to refresh subscriptions (subscribe to any new pages)
            debug!(user_did = %user_did, "User already connected, refreshing subscriptions");
            let _ = peer_actor.cast(PeerMessage::RefreshSubscriptions);
            return;
        }

        // 2. Resolve user_did to device info (node_id + permit)
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

        // 3. Parse node_id from string
        let node_id = match device_info.node_id.parse::<NodeId>() {
            Ok(id) => id,
            Err(e) => {
                warn!(node_id = %device_info.node_id, error = ?e, "Failed to parse node_id");
                return;
            }
        };

        // 4. Check if we're already connecting/connected to this node
        if state.peer_actors.contains_key(&node_id) || state.pending_connections.contains(&node_id) {
            debug!(user_did = %user_did, node_id = %node_id, "Already connected/connecting to node");
            return;
        }

        // 5. Initiate connection (use ConnectAndAuth pattern but ignore result)
        info!(user_did = %user_did, node_id = %node_id, "Initiating connection for sync");

        state.pending_connections.insert(node_id);
        state.pending_permits.insert(node_id, device_info.permit.clone());

        Self::emit_event(state, CourierEvent::ConnectRequested {
            node_id: node_id.to_string(),
            permit: device_info.permit,
        });
    }

    /// Find peer actor for a user (by any device).
    ///
    /// **Context**: Called to check if a user is already connected.
    fn get_peer_actor_for_user<'a>(
        &self,
        user_did: &str,
        state: &'a CoordinatorState,
    ) -> Option<&'a ActorRef<PeerMessage>> {
        state.authenticated_peers
            .iter()
            .find(|(_, info)| info.did == user_did)
            .map(|(node_id, _)| state.peer_actors.get(node_id))
            .flatten()
    }
}
