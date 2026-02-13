//! PeerActor - One actor per P2P connection
//!
//! Handles:
//! - Handshake state machine (Hello/Welcome/PermitGrant/Ack)
//! - Protocol message routing
//! - Per-peer state tracking
//!
//! # Handshake Flow (First Connection)
//!
//! ```text
//! Owner (User mode)                    Node (Node mode)
//!     │                                      │
//!     │──── Hello (first_connection) ───────>│
//!     │                                      │ validate permit
//!     │                                      │ store OwnerInfo
//!     │                                      │ issue permit_for_owner
//!     │<──────────── Welcome ────────────────│
//!     │ validate permit                      │
//!     │ store permit                         │
//!     │ issue permit_for_node                │
//!     │──────────── PermitGrant ────────────>│
//!     │                                      │ store permit
//!     │<──────────── Ack ────────────────────│
//!     │                                      │
//!   [Authenticated]                    [Authenticated]
//! ```

mod guards;
mod handshake;
mod publish;
mod sync;
mod consent;
mod assets;

use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Instant;

use logging_utils::short;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tokio::io::AsyncReadExt;
use tracing::{debug, error, info, warn, instrument};
use transport::{BiStream, Connection, NodeId};

use crate::coordinator::{CoordinatorMessage, CourierMode};
use crate::message::*;
use crate::state::{PeerState, PeerType};
use crate::trace::{MessageTrace, TraceDirection};
use butler::{Butler, BroadcastPayload, ScribeMessage};

/// Messages received by PeerActor
#[derive(Debug)]
pub enum PeerMessage {
    /// Protocol message from Coordinator (already deserialized)
    Protocol(Message),

    /// Internal: Initiate handshake by sending Hello (User mode)
    InitiateHandshake {
        permit: String,
    },

    /// Internal: Publish a space to this peer (User mode)
    PublishSpace {
        space_id: String,
    },

    /// Internal: Publish a page to this peer (User mode)
    PublishPage {
        page_id: String,
    },

    /// Internal: Request shareable link for a space (User mode)
    ///
    /// **Context**: Owner wants to share a space with a viewer.
    /// **We do**: Send GetShareableLinkRequest to node, wait for response.
    GetShareableLink {
        space_id: String,
        response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
    },

    /// Internal: Request space content as viewer (User mode)
    ///
    /// **Context**: Viewer has aud:* permit from shareable link.
    /// **We do**: Send SpaceRequest to node with permit.
    RequestSpace {
        space_id: String,
        viewer_permit: String,
    },

    /// Internal: Subscribe to a page's Scribe for live updates
    ///
    /// **Context**: After handshake, we want live updates for pages we have access to
    /// **We do**: Subscribe to Scribe, spawn broadcast listener task
    SubscribeToPage {
        page_id: String,
        permit: String,
    },

    /// Internal: Broadcast payload received from Scribe
    ///
    /// **Context**: Scribe sent us an update to forward to peer
    /// **We do**: Encrypt and send as SyncOffer
    BroadcastReceived(BroadcastPayload),

    /// Internal: Issue sync consent permits and send to node (User/Viewer mode)
    ///
    /// **Context**: Viewer received space + pages, now issues consent permits
    /// **We do**: Issue consent permits via gurkha, send SyncConsentGrant to node
    IssueSyncConsent {
        space_id: String,
        space_template: String,
        page_template: String,
    },

    /// Internal: Refresh subscriptions to active Scribes
    ///
    /// **Context**: Page opened after connection established
    /// **We do**: Re-run subscribe_to_active_scribes to pick up new pages
    RefreshSubscriptions,

    /// Raw datagram received from peer (ephemeral data)
    ///
    /// **Context**: PeerActor owns the datagram read loop
    /// **We do**: Deserialize, route to Scribe via page_subscriptions
    Datagram {
        data: Vec<u8>,
    },

    /// Internal: Send a layer permit to this peer (Node mode)
    ///
    /// **Context**: Coordinator received NewDynamicLayer event with ready permits
    /// **We do**: Send LayerPermitMsg to peer
    SendLayerPermit {
        page_id: String,
        layer_name: String,
        permit: String,
    },

    /// Internal: Send updated page permit to this peer (Node mode)
    ///
    /// **Context**: Page permit reissued with new app layers
    /// **We do**: Send PermitUpdate message to peer
    SendPermitUpdate {
        page_id: String,
        permit: String,
    },

    /// Internal: Request an asset from the peer
    ///
    /// **Context**: External component (e.g., Coordinator) wants to fetch an asset
    /// **We do**: Send AssetPrepare message to peer
    RequestAsset {
        page_id: String,
        hash: String,
    },

    /// Scribe initiates connection to receive messages
    ///
    /// **Context**: Scribe spawned and wants to receive ephemeral/sync messages
    /// **We do**: Store connection, flush any buffered messages for this page
    /// **Design**: Receiver-initiates pattern ensures no lost messages
    ScribeConnect {
        page_id: String,
        scribe: ractor::ActorRef<butler::ScribeMessage>,
    },
}

/// Blob store abstraction - either real transport or mock for testing
#[derive(Clone)]
pub enum BlobStore {
    /// Real iroh-blobs via Transport
    Real(Arc<transport::Transport>),
    /// Mock blob store for testing (shared HashMap)
    Mock(Arc<transport::MockBlobStore>),
}

impl BlobStore {
    /// Add blob to store, returns iroh hash bytes
    pub async fn add_blob(&self, data: &[u8]) -> Result<[u8; 32], String> {
        match self {
            BlobStore::Real(transport) => {
                transport.add_blob(data).await
                    .map(|h| *h.as_bytes())
                    .map_err(|e| format!("Blob store error: {}", e))
            }
            BlobStore::Mock(mock) => Ok(mock.add_blob(data)),
        }
    }

    /// Download blob from peer, returns plaintext bytes
    pub async fn download_blob(&self, iroh_hash: &[u8; 32], node_id: NodeId) -> Result<Vec<u8>, String> {
        match self {
            BlobStore::Real(transport) => {
                let hash = transport::BlobHash::from_bytes(*iroh_hash);
                transport.download_blob(hash, node_id).await
                    .map_err(|e| format!("Download failed: {}", e))
            }
            BlobStore::Mock(mock) => {
                mock.download_blob(iroh_hash)
                    .ok_or_else(|| "Blob not found in mock store".to_string())
            }
        }
    }
}

/// Arguments for spawning PeerActor
///
/// Generic over `C: Connection` to support different transport implementations.
pub struct PeerActorArgs<C: Connection> {
    pub mode: CourierMode,
    pub conn: C,
    pub coordinator: ActorRef<CoordinatorMessage<C>>,
    pub butler: Arc<Butler>,
    /// Blob store for asset transfer (real or mock)
    pub blob_store: BlobStore,
    /// Permit for outbound connections - if Some, auto-initiate handshake
    pub permit: Option<String>,
    /// Optional trace channel for protocol message capture (tests only)
    pub message_tx: Option<tokio::sync::mpsc::UnboundedSender<MessageTrace>>,
    /// Our node ID (for trace context)
    pub our_node_id: NodeId,
    /// Broadcast channel for capture system (pre-serialized JSON lines)
    pub capture_tx: Option<tokio::sync::broadcast::Sender<String>>,
}

// Unified Outbound Update (replaces BroadcastPayload + EphemeralOutbound)

/// Unified update enum for outbound sync/ephemeral data
///
/// **Context**: Single channel replaces separate broadcast_tx + ephemeral_tx
/// **Benefits**: One listener task instead of two per subscription
/// **Migration**: Use this to replace BroadcastPayload/EphemeralOutbound channels
#[derive(Debug, Clone)]
pub enum OutboundUpdate {
    /// CRDT sync update (3-step protocol)
    Sync {
        page_id: String,
        layer: String,
        update: Vec<u8>,
        state_vector: Vec<u8>,
    },
    /// Ephemeral data (cursor, typing, presence)
    Ephemeral {
        page_id: String,
        payload: Vec<u8>,
    },
}

/// Type alias for unified outbound update channel
pub type OutboundUpdateTx = tokio::sync::mpsc::Sender<OutboundUpdate>;

// Pending Operations

/// Pending viewer sync context (for streaming pages after SpaceDataAck)
#[derive(Debug, Clone)]
pub struct PendingViewerSync {
    /// Viewer's DID
    pub viewer_did: String,
    /// Viewer's public key (base64)
    pub viewer_pubkey_b64: String,
    /// Viewer's encryption key for transit encryption
    pub viewer_encryption_key: [u8; 32],
    /// Page IDs to stream
    pub page_ids: Vec<String>,
}

/// Context for pending SyncOffer operations (waiting for SyncAccept)
///
/// When we send a SyncOffer, we track the context so that when we receive
/// SyncAccept, we can compare state vectors and decide: SyncAck or resync.
#[derive(Debug, Clone)]
pub struct PendingSyncOffer {
    /// Our state vector at time of sending SyncOffer
    pub our_state_vector: Vec<u8>,
    /// The consent permit we used
    pub permit: String,
    /// Number of resync attempts for bounded retry
    /// After MAX_RESYNC_ATTEMPTS, we fall back to SyncReset
    pub resync_attempts: u8,
}

/// Maximum resync attempts before falling back to full snapshot
///
/// After this many SyncOffer/SyncAccept exchanges still show divergence,
/// we request full snapshot replacement (SyncReset → SyncSnapshot).
pub const MAX_RESYNC_ATTEMPTS: u8 = 3;

/// Active subscription to a page's Scribe
///
/// Stores all info needed to cleanly unsubscribe on disconnect.
pub struct PageSubscription {
    /// Scribe actor ref for sending Unsubscribe
    pub scribe: ActorRef<butler::ScribeMessage>,
    /// CRDT listener task handle (aborted on cleanup)
    pub listener_handle: tokio::task::JoinHandle<()>,
    /// Ephemeral listener task handle (aborted on cleanup)
    pub ephemeral_listener_handle: tokio::task::JoinHandle<()>,
    /// Subscriber's DID (for Unsubscribe message)
    pub user_did: String,
    /// Subscriber's device ID (for Unsubscribe message)
    pub device_id: String,
}

/// Pending asset transfer tracking for retry logic
///
/// **Context**: When we send AssetPrepare, track it for retry if AssetAck fails
/// **Retry**: Max 3 attempts with exponential backoff
#[derive(Debug, Clone)]
pub struct PendingAssetTransfer {
    /// Page containing the asset
    pub page_id: String,
    /// Number of retry attempts made
    pub attempts: u8,
    /// When the first attempt was started
    pub started: Instant,
}

/// Buffered message waiting for Scribe to connect
///
/// **Context**: When ephemeral/sync messages arrive before Scribe connects,
/// we buffer them here and flush when Scribe calls ScribeConnect.
#[derive(Debug, Clone)]
pub struct BufferedScribeMessage {
    /// Opaque payload (ephemeral data)
    pub payload: Vec<u8>,
    /// Sender's DID (if known from auth state)
    pub user_did: Option<String>,
    /// Sender's device ID (for relay exclusion)
    pub device_id: Option<String>,
}

/// Maximum retry attempts for asset transfers
const MAX_ASSET_TRANSFER_ATTEMPTS: u8 = 3;

/// Actor state for PeerActor
///
/// Generic over `C: Connection` to support different transport implementations.
pub struct PeerActorState<C: Connection> {
    /// Our node's mode (User or Node)
    mode: CourierMode,
    /// Current handshake state
    state: PeerState,
    /// Connection for sending messages (transport-agnostic)
    conn: C,
    /// Reference to coordinator for callbacks
    coordinator: ActorRef<CoordinatorMessage<C>>,
    /// Butler for storage operations
    butler: Arc<Butler>,
    /// Blob store for asset transfer (real or mock)
    blob_store: BlobStore,
    /// Pending viewer syncs by request_id (Node mode only)
    /// Stored when we send SpaceData, used when we receive SpaceDataAck
    pending_viewer_syncs: std::collections::HashMap<String, PendingViewerSync>,
    /// Peer's encryption key (set after handshake)
    /// Used for ECDH when sending SyncOffer messages
    peer_encryption_key: Option<[u8; 32]>,
    /// Cached parsed permit from handshake (Hello/Welcome/PermitGrant)
    /// Avoids re-parsing the permit on every message flow
    cached_peer_permit: Option<gurkha::Permit>,
    /// Active page subscriptions: page_id -> subscription info
    /// Used for explicit cleanup on disconnect (abort task + send Unsubscribe)
    page_subscriptions: std::collections::HashMap<String, PageSubscription>,
    /// Pending SyncOffer operations: (page_id, layer_name) -> context
    /// Tracks outgoing SyncOffers waiting for SyncAccept response
    pending_sync_offers: std::collections::HashMap<(String, String), PendingSyncOffer>,
    /// Pending asset transfers: hash -> transfer state
    /// Tracks outgoing AssetPrepare requests for retry on failure
    pending_asset_transfers: std::collections::HashMap<String, PendingAssetTransfer>,
    /// Pending shareable link requests: request_id -> response channel
    /// Tracks outgoing GetShareableLinkRequest waiting for response
    pending_shareable_link_requests: std::collections::HashMap<String, tokio::sync::oneshot::Sender<Result<String, String>>>,
    /// Direct Scribe connections: page_id -> Scribe actor ref
    ///
    /// **Context**: Scribe calls ScribeConnect on spawn, we store the connection here.
    /// Messages are forwarded directly to Scribe instead of via subscription pattern.
    /// **Design**: Receiver-initiates - Scribe connects to us, we don't push to Scribe.
    scribe_connections: std::collections::HashMap<String, ractor::ActorRef<butler::ScribeMessage>>,

    /// Buffered messages waiting for Scribe to connect: page_id -> Vec<BufferedScribeMessage>
    ///
    /// **Context**: When ephemeral arrives before Scribe connects, buffer here.
    /// Flushed when Scribe calls ScribeConnect.
    pending_scribe_messages: std::collections::HashMap<String, Vec<BufferedScribeMessage>>,

    /// Pending space request queued before handshake completes (User/Viewer mode)
    ///
    /// **Context**: Viewer sends SpaceRequest before Ack arrives (race condition).
    /// Queued here and drained in on_ack after handshake completes.
    pending_space_request: Option<(String, String)>,  // (space_id, viewer_permit)

    /// Optional trace channel for protocol message capture (tests only)
    message_tx: Option<tokio::sync::mpsc::UnboundedSender<MessageTrace>>,
    /// Our node ID (for trace context)
    our_node_id: NodeId,
    /// Broadcast channel for capture system (pre-serialized JSON lines)
    capture_tx: Option<tokio::sync::broadcast::Sender<String>>,
}

/// PeerActor handles one P2P connection
///
/// Generic over `C: Connection` to support different transport implementations:
/// - `IrohConnection`: Production transport with NAT traversal
/// - `MockConnection`: In-memory transport for unit tests
/// - `SimConnection`: Deterministic simulation for DST (future)
pub struct PeerActor<C: Connection> {
    node_id: NodeId,
    _phantom: PhantomData<C>,
}

impl<C: Connection> PeerActor<C> {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id, _phantom: PhantomData }
    }

    /// Send a message to the peer
    #[instrument(skip_all, fields(node = %short(&self.node_id), msg = %msg.name()))]
    async fn send_message(&self, msg: &Message, state: &PeerActorState<C>) {
        let bytes = match msg.to_bytes() {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to serialize message: {}", e);
                return;
            }
        };

        // Emit to trace channel (tests) and capture channel (observability)
        let mut trace = MessageTrace::new(
            TraceDirection::Sent,
            msg.name(),
            state.our_node_id,
            self.node_id,
        );
        let (ctx_page_id, ctx_layer) = msg.context();
        trace.page_id = ctx_page_id.map(String::from);
        trace.layer_name = ctx_layer.map(String::from);
        if let Some(tx) = &state.capture_tx {
            let mut json = serde_json::json!({
                "type": "message_trace",
                "ts": &trace.ts,
                "direction": "Sent",
                "msg": trace.msg_name,
                "node_id": trace.node_id.to_string(),
                "peer_node_id": trace.peer_node_id.to_string(),
            });
            if let Some(ref pid) = trace.page_id {
                json["page_id"] = serde_json::json!(pid);
            }
            if let Some(ref ln) = trace.layer_name {
                json["layer"] = serde_json::json!(ln);
            }
            let _ = tx.send(json.to_string());
        }
        if let Some(tx) = &state.message_tx {
            let _ = tx.send(trace);
        }

        if let Err(e) = state.conn.send_bytes(&bytes).await {
            error!("Failed to send message to {}: {}", self.node_id, e);
        }
    }

    /// Send AssetPrepare message and track for retry on failure
    ///
    /// **Context**: We want to request an asset from peer
    /// **We do**: Track the request, send AssetPrepare message
    #[instrument(skip_all, fields(node = %short(&self.node_id), page_id = %page_id, hash = %hash))]
    pub async fn send_asset_prepare(&self, page_id: &str, hash: &str, state: &mut PeerActorState<C>) {
        // Track the transfer for retry logic
        state.pending_asset_transfers.insert(
            hash.to_string(),
            PendingAssetTransfer {
                page_id: page_id.to_string(),
                attempts: 0,
                started: Instant::now(),
            },
        );

        // Send the request
        self.send_message(&Message::AssetPrepare(AssetPrepareMsg {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
        }), state).await;

        debug!(page_id = %page_id, hash = %hash, "Sent AssetPrepare (tracked for retry)");
    }

    /// Send rejection and transition to failed state
    #[instrument(skip_all, fields(node = %short(&self.node_id), reason = %reason))]
    async fn reject(&self, reason: &str, state: &mut PeerActorState<C>) {
        warn!("Rejecting peer {}: {}", self.node_id, reason);
        self.send_message(&Message::Rejected(RejectedMsg { reason: reason.to_string() }), state).await;
        state.state = PeerState::fail(reason);
        self.notify_failed(state, reason);
    }

    /// Parse permit or send error response on failure
    async fn parse_permit_or_respond(
        &self,
        token: &str,
        error_msg: Message,
        state: &PeerActorState<C>,
    ) -> Option<gurkha::Permit> {
        match gurkha::Permit::from_token(token) {
            Ok(p) => Some(p),
            Err(e) => {
                error!("Invalid permit: {:?}", e);
                self.send_message(&error_msg, state).await;
                None
            }
        }
    }

    /// Notify coordinator of authentication
    fn notify_authenticated(&self, state: &PeerActorState<C>, peer_type: PeerType, did: &str, username: &str) {
        // Emit peer_identity event for capture system (DID → human-readable mapping)
        if let Some(ref tx) = state.capture_tx {
            let short_did = &did[did.len().saturating_sub(12)..];
            let _ = tx.send(serde_json::json!({
                "type": "peer_identity",
                "ts": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                "node_id": self.node_id.to_string(),
                "peer_type": format!("{:?}", peer_type),
                "did": did,
                "short_did": short_did,
                "username": username,
            }).to_string());
        }

        let _ = state.coordinator.cast(CoordinatorMessage::PeerAuthenticated {
            node_id: self.node_id,
            peer_type,
            did: did.to_string(),
            username: username.to_string(),
        });
    }

    /// Notify coordinator of failure
    fn notify_failed(&self, state: &PeerActorState<C>, reason: &str) {
        let _ = state.coordinator.cast(CoordinatorMessage::PeerFailed {
            node_id: self.node_id,
            reason: reason.to_string(),
        });
    }
}

impl<C: Connection> Actor for PeerActor<C> {
    type Msg = PeerMessage;
    type State = PeerActorState<C>;
    type Arguments = PeerActorArgs<C>;

    #[instrument(skip_all, fields(node = %self.node_id))]
    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("PeerActor started for {} in {:?} mode", self.node_id, args.mode);

        // Spawn datagram read loop - routes ephemeral data to self
        let conn_for_datagrams = args.conn.clone();
        let myself_for_datagrams = myself.clone();
        let node_id = self.node_id;
        tokio::spawn(async move {
            debug!(node_id = %node_id, "Datagram read loop started");
            loop {
                match conn_for_datagrams.read_datagram().await {
                    Ok(data) => {
                        debug!(node_id = %node_id, data_len = data.len(), "Datagram read loop: received datagram");
                        if myself_for_datagrams.cast(PeerMessage::Datagram { data: data.to_vec() }).is_err() {
                            debug!("PeerActor gone, stopping datagram read loop for {}", node_id);
                            break;
                        }
                    }
                    Err(_) => {
                        // Connection closed
                        debug!("Datagram read loop ending for {} (connection closed)", node_id);
                        break;
                    }
                }
            }
        });

        // Stream read loop - accepts bidirectional streams and processes protocol messages
        let conn_for_streams = args.conn.clone();
        let myself_for_streams = myself.clone();
        let node_id_for_streams = self.node_id;
        let coordinator_for_disconnect = args.coordinator.clone();
        tokio::spawn(async move {
            loop {
                let bi_stream = match conn_for_streams.accept_bi().await {
                    Ok(streams) => streams,
                    Err(_) => {
                        let _ = coordinator_for_disconnect.cast(CoordinatorMessage::Disconnected {
                            node_id: node_id_for_streams
                        });
                        break;
                    }
                };

                let (_, mut recv) = bi_stream.split();

                // Read length-prefixed message
                let mut len_buf = [0u8; 4];
                if recv.read_exact(&mut len_buf).await.is_err() {
                    continue;
                }

                let len = u32::from_be_bytes(len_buf) as usize;
                if len > 10 * 1024 * 1024 {
                    continue;
                }

                let mut data = vec![0u8; len];
                if recv.read_exact(&mut data).await.is_err() {
                    continue;
                }

                // Deserialize and process directly
                match Message::from_bytes(&data) {
                    Ok(msg) => {
                        if myself_for_streams.cast(PeerMessage::Protocol(msg)).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("Failed to deserialize message: {}", e);
                    }
                }
            }
        });

        // If permit provided (outbound connection), auto-initiate handshake
        if let Some(permit) = args.permit {
            info!("Auto-initiating handshake with permit for {}", self.node_id);
            let _ = myself.cast(PeerMessage::InitiateHandshake { permit });
        }

        Ok(PeerActorState {
            mode: args.mode,
            state: PeerState::Connected,
            conn: args.conn,
            coordinator: args.coordinator,
            butler: args.butler,
            blob_store: args.blob_store,
            pending_viewer_syncs: std::collections::HashMap::new(),
            peer_encryption_key: None,
            cached_peer_permit: None,
            page_subscriptions: std::collections::HashMap::new(),
            pending_sync_offers: std::collections::HashMap::new(),
            pending_asset_transfers: std::collections::HashMap::new(),
            pending_shareable_link_requests: std::collections::HashMap::new(),
            scribe_connections: std::collections::HashMap::new(),
            pending_scribe_messages: std::collections::HashMap::new(),
            pending_space_request: None,
            message_tx: args.message_tx,
            our_node_id: args.our_node_id,
            capture_tx: args.capture_tx,
        })
    }

    #[instrument(skip_all, fields(node = %self.node_id))]
    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            PeerMessage::Protocol(msg) => {
                self.handle_protocol_message(myself, msg, state).await;
            }

            PeerMessage::InitiateHandshake { permit } => {
                self.initiate_handshake(&permit, state).await;
            }

            PeerMessage::PublishSpace { space_id } => {
                self.initiate_publish_space(&space_id, state).await;
            }

            PeerMessage::PublishPage { page_id } => {
                self.initiate_page_announce(&page_id, state).await;
            }

            PeerMessage::GetShareableLink { space_id, response_tx } => {
                self.initiate_get_shareable_link(&space_id, response_tx, state).await;
            }

            PeerMessage::RequestSpace { space_id, viewer_permit } => {
                // Queue if handshake not complete yet (race: SpaceRequest sent before Ack)
                if !matches!(state.state, PeerState::Authenticated { .. }) {
                    info!("Queueing SpaceRequest for {} until handshake completes", space_id);
                    state.pending_space_request = Some((space_id, viewer_permit));
                } else {
                    self.initiate_request_space_as_viewer(&space_id, &viewer_permit, state).await;
                }
            }

            PeerMessage::SubscribeToPage { page_id, permit } => {
                self.subscribe_to_page(myself, &page_id, &permit, state).await;
            }

            PeerMessage::BroadcastReceived(payload) => {
                self.handle_broadcast_received(payload, state).await;
            }

            PeerMessage::IssueSyncConsent { space_id, space_template, page_template } => {
                self.issue_sync_consent(&space_id, &space_template, &page_template, state).await;
            }

            PeerMessage::RefreshSubscriptions => {
                info!(node_id = %self.node_id, "RefreshSubscriptions received");
                // First check health of existing subscriptions (resubscribe if dead)
                self.check_subscription_health(myself.clone(), state).await;
                // Then subscribe to any new active scribes
                self.subscribe_to_active_scribes(myself, state).await;
            }

            PeerMessage::Datagram { data } => {
                self.handle_datagram(&data, state).await;
            }

            PeerMessage::SendLayerPermit { page_id, layer_name, permit } => {
                self.send_layer_permit(&page_id, &layer_name, &permit, state).await;
            }

            PeerMessage::SendPermitUpdate { page_id, permit } => {
                self.send_page_permit_update(&page_id, &permit, state).await;
            }

            PeerMessage::RequestAsset { page_id, hash } => {
                info!(page_id = %page_id, hash = %hash, "RequestAsset: sending AssetPrepare to peer");
                self.send_asset_prepare(&page_id, &hash, state).await;
            }

            PeerMessage::ScribeConnect { page_id, scribe } => {
                self.handle_scribe_connect(&page_id, scribe, state).await;
            }
        }

        Ok(())
    }

    #[instrument(skip_all, fields(node = %self.node_id))]
    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let subscription_count = state.page_subscriptions.len();
        let scribe_connection_count = state.scribe_connections.len();

        // Clean up all page subscriptions
        for (page_id, subscription) in state.page_subscriptions.drain() {
            // Abort both listener tasks to prevent orphaned tasks on closed connections
            subscription.listener_handle.abort();
            subscription.ephemeral_listener_handle.abort();

            // Send Unsubscribe to Scribe
            if let Err(e) = subscription.scribe.cast(butler::ScribeMessage::Unsubscribe {
                user_did: subscription.user_did.clone(),
                device_id: subscription.device_id.clone(),
            }) {
                warn!("Failed to send Unsubscribe for page {}: {}", page_id, e);
            } else {
                debug!("Unsubscribed from page {} (user: {}, device: {})",
                    page_id, subscription.user_did, subscription.device_id);
            }
        }

        // Clear direct Scribe connections
        state.scribe_connections.clear();

        info!("PeerActor stopped for {} ({} subscriptions, {} scribe connections cleaned up)",
            self.node_id, subscription_count, scribe_connection_count);
        Ok(())
    }
}

impl<C: Connection> PeerActor<C> {
    /// Handle protocol message
    #[instrument(skip(self, myself, message, state), fields(node = %short(&self.node_id), peer_state = %state.state.name(), msg = %message.name()))]
    async fn handle_protocol_message(
        &self,
        myself: ActorRef<PeerMessage>,
        message: Message,
        state: &mut PeerActorState<C>,
    ) {
        // Emit to trace channel (tests) and capture channel (observability)
        let mut trace = MessageTrace::new(
            TraceDirection::Received,
            message.name(),
            state.our_node_id,
            self.node_id,
        );
        let (ctx_page_id, ctx_layer) = message.context();
        trace.page_id = ctx_page_id.map(String::from);
        trace.layer_name = ctx_layer.map(String::from);
        if let Some(tx) = &state.capture_tx {
            let mut json = serde_json::json!({
                "type": "message_trace",
                "ts": &trace.ts,
                "direction": "Received",
                "msg": trace.msg_name,
                "node_id": trace.node_id.to_string(),
                "peer_node_id": trace.peer_node_id.to_string(),
            });
            if let Some(ref pid) = trace.page_id {
                json["page_id"] = serde_json::json!(pid);
            }
            if let Some(ref ln) = trace.layer_name {
                json["layer"] = serde_json::json!(ln);
            }
            let _ = tx.send(json.to_string());
        }
        if let Some(tx) = &state.message_tx {
            let _ = tx.send(trace);
        }

        match message {
            Message::Hello(m) => {
                self.on_hello(&m.did, &m.username, &m.public_key, &m.encryption_key, &m.permit, state).await;
            }

            Message::Welcome(m) => {
                self.on_welcome(myself.clone(), &m.permit_for_peer, &m.node_public_key, &m.node_encryption_key, state).await;
            }

            Message::PermitGrant(m) => {
                self.on_permit_grant(myself.clone(), &m.permit_for_node, state).await;
            }

            Message::Ack => {
                debug!("Received Ack from {}", self.node_id);
                self.on_ack(myself.clone(), state).await;
            }

            Message::Rejected(m) => {
                warn!("Connection rejected by {}: {}", self.node_id, m.reason);
                state.state = PeerState::fail(&m.reason);
                self.notify_failed(state, &m.reason);
            }

            // Publishing messages
            Message::PublishSpace(m) => {
                self.on_publish_space(&m.request_id, &m.space, &m.space_permit, state).await;
            }

            Message::PageAnnounce(m) => {
                self.on_page_announce(&m.request_id, &m.page, &m.page_permit, &m.owner_permit, state).await;
            }

            Message::PageAnnounceAck(m) => {
                self.on_page_announce_ack(myself.clone(), &m.request_id, &m.page_id, &m.permit, state).await;
            }

            Message::PublishSpaceAck(m) => {
                self.on_publish_space_ack(&m.request_id, &m.permit, &m.pages, state).await;
            }

            Message::PublishError(m) => {
                self.on_publish_error(&m.request_id, &m.error, state).await;
            }

            Message::PermitUpdate(m) => {
                self.on_permit_update(&m.permit, &m.scope, state).await;
            }

            // 3-Step Sync Protocol messages
            Message::SyncOffer(m) => {
                self.on_sync_offer(
                    myself.clone(),
                    &m.page_id,
                    &m.layer_name,
                    &m.data,
                    &m.state_vector,
                    &m.ephemeral_public,
                    m.authority_permit.as_deref(),
                    state,
                )
                .await;
            }

            Message::SyncAccept(m) => {
                self.on_sync_accept(&m.page_id, &m.layer_name, &m.state_vector, state).await;
            }

            Message::SyncAck(m) => {
                self.on_sync_ack(&m.page_id, &m.layer_name, &m.state_vector, state).await;
            }

            Message::SyncReset(m) => {
                self.on_sync_reset(&m.page_id, &m.layer_name, state).await;
            }

            Message::SyncSnapshot(m) => {
                self.on_sync_snapshot(&m.page_id, &m.layer_name, &m.snapshot, &m.state_vector, &m.ephemeral_public, state).await;
            }

            // Shareable link messages
            Message::GetShareableLinkRequest(m) => {
                self.on_get_shareable_link_request(&m.request_id, &m.space_id, state).await;
            }

            Message::GetShareableLinkResponse(m) => {
                self.on_get_shareable_link_response(&m.request_id, &m.space_id, &m.permit, state).await;
            }

            // Viewer space request messages
            Message::SpaceRequest(m) => {
                self.on_space_request(&m.request_id, &m.space_id, &m.viewer_did, &m.viewer_public_key, &m.viewer_encryption_key, &m.viewer_permit, state).await;
            }

            Message::SpaceData(m) => {
                self.on_space_data(&m.request_id, &m.space_id, &m.delegated_permit, &m.space, &m.pages, state).await;
            }

            Message::SpaceDataAck(m) => {
                self.on_space_data_ack(myself.clone(), &m.request_id, &m.space_id, &m.delegated_permit, state).await;
            }

            Message::SpaceRequestError(m) => {
                error!("SpaceRequestError: request={} error={}", m.request_id, m.error);
            }

            // Sync consent messages
            Message::SyncConsentGrant(m) => {
                self.on_sync_consent_grant(&m.request_id, &m.space_id, &m.space_consent_permit, &m.page_consent_permits, state).await;
            }

            Message::SyncConsentAck(m) => {
                self.on_sync_consent_ack(&m.request_id, &m.space_id, state).await;
            }

            // Layer permit + consent messages
            Message::LayerPermit(m) => {
                self.on_layer_permit(&m.request_id, &m.page_id, &m.layer_name, &m.permit, state).await;
            }

            Message::LayerConsentGrant(m) => {
                self.on_layer_consent_grant(&m.request_id, &m.page_id, &m.layer_name, &m.consent_permit, state).await;
            }

            Message::LayerConsentAck(m) => {
                self.on_layer_consent_ack(&m.request_id, &m.page_id, &m.layer_name, state).await;
            }

            // Asset sync messages
            Message::AssetPrepare(m) => {
                self.on_asset_prepare(&m.page_id, &m.hash, state).await;
            }

            Message::AssetReady(m) => {
                self.on_asset_ready(&m.page_id, &m.hash, &m.iroh_hash, state).await;
            }

            Message::AssetAck(m) => {
                self.on_asset_ack(&m.page_id, &m.hash, m.success, m.error.as_deref(), state).await;
            }

            Message::Error(m) => {
                error!(
                    "Error from {}: {:?} - {} (id: {:?})",
                    self.node_id, m.code, m.message, m.id
                );
            }
        }
    }

    /// Handle ephemeral datagram received from peer
    ///
    /// **Context**: Datagram received via PeerActor's read loop
    /// **We do**: Check for Scribe connection, forward directly or buffer
    /// **Design**: Receiver-initiates pattern - Scribe connects to us via ScribeConnect
    #[instrument(skip_all, fields(node = %short(&self.node_id), data_len = data.len()))]
    async fn handle_datagram(&self, data: &[u8], state: &mut PeerActorState<C>) {
        debug!(node_id = %self.node_id, data_len = data.len(), "handle_datagram: received datagram from peer");

        // Deserialize the datagram
        let datagram = match EphemeralDatagram::from_bytes(data) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to deserialize ephemeral datagram from {}: {}", self.node_id, e);
                return;
            }
        };

        debug!(
            node_id = %self.node_id,
            page_id = %datagram.page_id,
            scribe_connections = ?state.scribe_connections.keys().collect::<Vec<_>>(),
            "handle_datagram: deserialized, checking for Scribe connection"
        );

        // Get peer's DID from state (set during handshake)
        let user_did = state.state.did().map(String::from);
        // Use node_id as device_id for sender exclusion during relay
        let device_id = Some(self.node_id.to_string());

        // Check if we have a direct Scribe connection for this page
        if let Some(scribe) = state.scribe_connections.get(&datagram.page_id) {
            // Fast path: forward directly to Scribe
            match scribe.cast(ScribeMessage::RemoteEphemeral {
                user_did: user_did.clone(),
                device_id: device_id.clone(),
                payload: datagram.payload.clone(),
            }) {
                Ok(()) => {
                    debug!(
                        page_id = %datagram.page_id,
                        from_did = ?user_did,
                        "Ephemeral forwarded to Scribe via direct connection"
                    );
                }
                Err(e) => {
                    warn!("Failed to forward ephemeral to Scribe: {}", e);
                    // Scribe might have stopped, remove connection
                    state.scribe_connections.remove(&datagram.page_id);
                }
            }
            return;
        }

        // No Scribe connection yet - buffer the message
        info!(
            page_id = %datagram.page_id,
            from_did = ?user_did,
            "No Scribe connection - buffering ephemeral for later delivery"
        );

        state.pending_scribe_messages
            .entry(datagram.page_id.clone())
            .or_default()
            .push(BufferedScribeMessage {
                payload: datagram.payload,
                user_did,
                device_id,
            });
    }

    /// Handle Scribe connection request
    ///
    /// **Context**: Scribe spawned and wants to receive messages for a page
    /// **We do**: Store connection, flush any buffered messages
    /// **Design**: Receiver-initiates pattern ensures no lost messages
    #[instrument(skip_all, fields(node = %short(&self.node_id), page_id = %page_id))]
    async fn handle_scribe_connect(
        &self,
        page_id: &str,
        scribe: ractor::ActorRef<butler::ScribeMessage>,
        state: &mut PeerActorState<C>,
    ) {
        // Flush any buffered messages for this page
        if let Some(buffered) = state.pending_scribe_messages.remove(page_id) {
            let count = buffered.len();
            info!(
                page_id = %page_id,
                buffered_count = count,
                "ScribeConnect received, flushing {} buffered messages",
                count
            );

            for msg in buffered {
                if let Err(e) = scribe.cast(ScribeMessage::RemoteEphemeral {
                    user_did: msg.user_did,
                    device_id: msg.device_id,
                    payload: msg.payload,
                }) {
                    warn!(page_id = %page_id, error = %e, "Failed to flush buffered message");
                } else {
                    debug!(page_id = %page_id, "Flushed buffered message to Scribe");
                }
            }
        } else {
            info!(page_id = %page_id, "ScribeConnect received, no buffered messages");
        }

        // Store the connection for future messages
        state.scribe_connections.insert(page_id.to_string(), scribe);
    }
}
