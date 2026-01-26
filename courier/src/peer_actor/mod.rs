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

use std::sync::Arc;
use std::time::Instant;

use logging_utils::short;
use ractor::{Actor, ActorProcessingErr, ActorRef};
use tracing::{debug, error, info, warn, instrument};
use transport::{ConnectionHandle, NodeId};

use crate::coordinator::{CoordinatorMessage, CourierMode};
use crate::message::{Message, EphemeralDatagram};
use crate::state::{PeerState, PeerType};
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

    /// Internal: Request an asset from the peer
    ///
    /// **Context**: External component (e.g., Coordinator) wants to fetch an asset
    /// **We do**: Send AssetPrepare message to peer
    RequestAsset {
        page_id: String,
        hash: String,
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

/// Arguments for spawning PeerActor
pub struct PeerActorArgs {
    pub mode: CourierMode,
    pub conn: ConnectionHandle,
    pub coordinator: ActorRef<CoordinatorMessage>,
    pub butler: Arc<Butler>,
    /// Blob store for asset transfer (real or mock)
    pub blob_store: BlobStore,
    /// Permit for outbound connections - if Some, auto-initiate handshake
    pub permit: Option<String>,
}

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
}

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

/// Maximum retry attempts for asset transfers
const MAX_ASSET_TRANSFER_ATTEMPTS: u8 = 3;

/// Actor state for PeerActor
pub struct PeerActorState {
    /// Our node's mode (User or Node)
    mode: CourierMode,
    /// Current handshake state
    state: PeerState,
    /// Connection handle for sending messages
    conn: ConnectionHandle,
    /// Reference to coordinator for callbacks
    coordinator: ActorRef<CoordinatorMessage>,
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
}

/// PeerActor handles one P2P connection
pub struct PeerActor {
    node_id: NodeId,
}

impl PeerActor {
    pub fn new(node_id: NodeId) -> Self {
        Self { node_id }
    }

    /// Send a message to the peer
    async fn send_message(&self, msg: &Message, state: &PeerActorState) {
        let bytes = match msg.to_bytes() {
            Ok(b) => b,
            Err(e) => {
                error!("Failed to serialize message: {}", e);
                return;
            }
        };

        if let Err(e) = state.conn.send_bytes(&bytes).await {
            error!("Failed to send message to {}: {}", self.node_id, e);
        }
    }

    /// Send AssetPrepare message and track for retry on failure
    ///
    /// **Context**: We want to request an asset from peer
    /// **We do**: Track the request, send AssetPrepare message
    pub async fn send_asset_prepare(&self, page_id: &str, hash: &str, state: &mut PeerActorState) {
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
        self.send_message(&Message::AssetPrepare {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
        }, state).await;

        debug!(page_id = %page_id, hash = %hash, "Sent AssetPrepare (tracked for retry)");
    }

    /// Send rejection and transition to failed state
    async fn reject(&self, reason: &str, state: &mut PeerActorState) {
        warn!("Rejecting peer {}: {}", self.node_id, reason);
        self.send_message(&Message::Rejected { reason: reason.to_string() }, state).await;
        state.state = PeerState::fail(reason);
        self.notify_failed(state, reason);
    }

    /// Notify coordinator of authentication
    fn notify_authenticated(&self, state: &PeerActorState, peer_type: PeerType, did: &str, username: &str) {
        let _ = state.coordinator.cast(CoordinatorMessage::PeerAuthenticated {
            node_id: self.node_id,
            peer_type,
            did: did.to_string(),
            username: username.to_string(),
        });
    }

    /// Notify coordinator of failure
    fn notify_failed(&self, state: &PeerActorState, reason: &str) {
        let _ = state.coordinator.cast(CoordinatorMessage::PeerFailed {
            node_id: self.node_id,
            reason: reason.to_string(),
        });
    }
}

#[cfg_attr(feature = "async-trait", ractor::async_trait)]
impl Actor for PeerActor {
    type Msg = PeerMessage;
    type State = PeerActorState;
    type Arguments = PeerActorArgs;

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
            loop {
                match conn_for_datagrams.read_datagram().await {
                    Ok(data) => {
                        if myself_for_datagrams.cast(PeerMessage::Datagram { data }).is_err() {
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
                let (_, mut recv) = match conn_for_streams.accept_bi().await {
                    Ok(streams) => streams,
                    Err(_) => {
                        let _ = coordinator_for_disconnect.cast(CoordinatorMessage::Disconnected {
                            node_id: node_id_for_streams
                        });
                        break;
                    }
                };

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
            page_subscriptions: std::collections::HashMap::new(),
            pending_sync_offers: std::collections::HashMap::new(),
            pending_asset_transfers: std::collections::HashMap::new(),
            pending_shareable_link_requests: std::collections::HashMap::new(),
        })
    }

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
                self.initiate_publish_page(&page_id, state).await;
            }

            PeerMessage::GetShareableLink { space_id, response_tx } => {
                self.initiate_get_shareable_link(&space_id, response_tx, state).await;
            }

            PeerMessage::RequestSpace { space_id, viewer_permit } => {
                self.initiate_request_space_as_viewer(&space_id, &viewer_permit, state).await;
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
                self.subscribe_to_active_scribes(myself, state).await;
            }

            PeerMessage::Datagram { data } => {
                self.handle_datagram(&data, state).await;
            }

            PeerMessage::RequestAsset { page_id, hash } => {
                info!(page_id = %page_id, hash = %hash, "RequestAsset: sending AssetPrepare to peer");
                self.send_asset_prepare(&page_id, &hash, state).await;
            }
        }

        Ok(())
    }

    async fn post_stop(
        &self,
        _myself: ActorRef<Self::Msg>,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        let subscription_count = state.page_subscriptions.len();

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

        info!("PeerActor stopped for {} ({} subscriptions cleaned up)",
            self.node_id, subscription_count);
        Ok(())
    }
}

impl PeerActor {
    /// Handle protocol message
    #[instrument(skip(self, myself, message, state), fields(node = %short(&self.node_id), peer_state = %state.state.name(), msg = %message.name()))]
    async fn handle_protocol_message(
        &self,
        myself: ActorRef<PeerMessage>,
        message: Message,
        state: &mut PeerActorState,
    ) {
        // Message name logged in instrument fields above

        match message {
            Message::Hello {
                did,
                username,
                public_key,
                encryption_key,
                signature: _,
                timestamp: _,
                permit,
            } => {
                self.on_hello(&did, &username, &public_key, &encryption_key, &permit, state).await;
            }

            Message::Welcome {
                node_id: _,
                node_public_key,
                node_encryption_key,
                signature: _,
                timestamp: _,
                permit_for_peer,
            } => {
                self.on_welcome(myself.clone(), &permit_for_peer, &node_public_key, &node_encryption_key, state).await;
            }

            Message::PermitGrant { permit_for_node } => {
                self.on_permit_grant(myself.clone(), &permit_for_node, state).await;
            }

            Message::Ack => {
                debug!("Received Ack from {}", self.node_id);
                self.on_ack(myself.clone(), state).await;
            }

            Message::Rejected { reason } => {
                warn!("Connection rejected by {}: {}", self.node_id, reason);
                state.state = PeerState::fail(&reason);
                self.notify_failed(state, &reason);
            }

            // Publishing messages - require authentication
            Message::PublishSpace { request_id, space, space_permit } => {
                self.on_publish_space(&request_id, &space, &space_permit, state).await;
            }

            Message::PublishPage { request_id, page, page_permit, owner_permit, ephemeral_public, layers } => {
                self.on_publish_page(&request_id, &page, &page_permit, &owner_permit, &ephemeral_public, &layers, state).await;
            }

            Message::PublishPageAck { request_id, page_id, permit } => {
                self.on_publish_page_ack(&request_id, &page_id, &permit, state).await;
            }

            Message::PublishSpaceAck { request_id, permit, pages } => {
                self.on_publish_space_ack(&request_id, &permit, &pages, state).await;
            }

            Message::PublishError { request_id, error } => {
                self.on_publish_error(&request_id, &error, state).await;
            }

            // 3-Step Sync Protocol messages
            Message::SyncOffer { page_id, layer_name, data, state_vector, ephemeral_public, permit } => {
                self.on_sync_offer(myself.clone(), &page_id, &layer_name, &data, &state_vector, &ephemeral_public, &permit, state).await;
            }

            Message::SyncAccept { page_id, layer_name, state_vector } => {
                self.on_sync_accept(&page_id, &layer_name, &state_vector, state).await;
            }

            Message::SyncAck { page_id, layer_name, state_vector } => {
                self.on_sync_ack(&page_id, &layer_name, &state_vector, state).await;
            }

            // Shareable link messages
            Message::GetShareableLinkRequest { request_id, space_id } => {
                self.on_get_shareable_link_request(&request_id, &space_id, state).await;
            }

            Message::GetShareableLinkResponse { request_id, space_id, permit } => {
                self.on_get_shareable_link_response(&request_id, &space_id, &permit, state).await;
            }

            // Viewer space request messages
            Message::SpaceRequest { request_id, space_id, viewer_did, viewer_public_key, viewer_encryption_key, viewer_permit } => {
                self.on_space_request(&request_id, &space_id, &viewer_did, &viewer_public_key, &viewer_encryption_key, &viewer_permit, state).await;
            }

            Message::SpaceData { request_id, space_id, delegated_permit, space, page_ids } => {
                self.on_space_data(&request_id, &space_id, &delegated_permit, &space, &page_ids, state).await;
            }

            Message::SpaceDataAck { request_id, space_id, delegated_permit } => {
                self.on_space_data_ack(&request_id, &space_id, &delegated_permit, state).await;
            }

            Message::PageData { request_id, space_id, meta, permit, ephemeral_public, layers, is_last } => {
                self.on_viewer_page(&request_id, &space_id, &meta, &permit, &ephemeral_public, &layers, is_last, state).await;
            }

            Message::SpaceRequestError { request_id, error } => {
                error!("SpaceRequestError: request={} error={}", request_id, error);
            }

            // Sync consent messages (Viewer → Node → Viewer)
            Message::SyncConsentGrant { request_id, space_id, space_consent_permit, page_consent_permits } => {
                self.on_sync_consent_grant(&request_id, &space_id, &space_consent_permit, &page_consent_permits, state).await;
            }

            Message::SyncConsentAck { request_id, space_id } => {
                self.on_sync_consent_ack(&request_id, &space_id, state).await;
            }

            // Asset sync messages (iroh-blobs)
            Message::AssetPrepare { page_id, hash } => {
                self.on_asset_prepare(&page_id, &hash, state).await;
            }

            Message::AssetReady { page_id, hash, iroh_hash } => {
                self.on_asset_ready(&page_id, &hash, &iroh_hash, state).await;
            }

            Message::AssetAck { page_id, hash, success, error } => {
                self.on_asset_ack(&page_id, &hash, success, error.as_deref(), state).await;
            }

            Message::Error { id, code, message } => {
                error!(
                    "Error from {}: {:?} - {} (id: {:?})",
                    self.node_id, code, message, id
                );
            }
        }
    }

    // ========================================================================
    // Asset Sync Handlers
    // ========================================================================

    /// Handle AssetPrepare request from peer
    ///
    /// **Context**: Peer wants us to prepare an asset for transfer
    /// **We do**: Decrypt from local storage, add to iroh-blobs, send AssetReady
    async fn on_asset_prepare(&self, page_id: &str, hash: &str, state: &mut PeerActorState) {
        info!(
            page_id = %page_id,
            hash = %hash,
            peer = %self.node_id,
            "AssetPrepare received - preparing blob for transfer"
        );

        // 1. Get page key via butler
        let (_, page_key) = match state.butler.get_decrypted_page(page_id).await {
            Ok(result) => result,
            Err(e) => {
                warn!(page_id = %page_id, error = ?e, "Failed to get page key for asset");
                self.send_message(&Message::AssetAck {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                    success: false,
                    error: Some(format!("Page not found: {}", e)),
                }, state).await;
                return;
            }
        };

        // 2. Get plaintext via asset service
        let plaintext = match butler::services::asset_service::get_for_transfer(
            state.butler.asset_store(),
            &page_key,
            hash,
        ) {
            Ok(data) => data,
            Err(e) => {
                warn!(hash = %hash, error = ?e, "Failed to get asset for transfer");
                self.send_message(&Message::AssetAck {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                    success: false,
                    error: Some(format!("Asset not found: {}", e)),
                }, state).await;
                return;
            }
        };

        // 3. Add to blob store (real transport or mock)
        let iroh_hash_bytes: [u8; 32] = match &state.blob_store {
            BlobStore::Real(transport) => {
                match transport.add_blob(&plaintext).await {
                    Ok(h) => *h.as_bytes(),
                    Err(e) => {
                        warn!(hash = %hash, error = ?e, "Failed to add blob to transport");
                        self.send_message(&Message::AssetAck {
                            page_id: page_id.to_string(),
                            hash: hash.to_string(),
                            success: false,
                            error: Some(format!("Blob store error: {}", e)),
                        }, state).await;
                        return;
                    }
                }
            }
            BlobStore::Mock(mock_store) => {
                mock_store.add_blob(&plaintext)
            }
        };

        info!(
            page_id = %page_id,
            hash = %hash,
            iroh_hash = %hex::encode(&iroh_hash_bytes),
            size = plaintext.len(),
            "Asset prepared for transfer"
        );

        // 4. Send AssetReady with iroh_hash
        self.send_message(&Message::AssetReady {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
            iroh_hash: iroh_hash_bytes,
        }, state).await;
    }

    /// Handle AssetReady notification from peer
    ///
    /// **Context**: Peer has prepared asset, blob is ready for download
    /// **We do**: Download via iroh-blobs, verify, encrypt, store locally
    async fn on_asset_ready(
        &self,
        page_id: &str,
        hash: &str,
        iroh_hash_bytes: &[u8; 32],
        state: &mut PeerActorState,
    ) {
        info!(
            page_id = %page_id,
            hash = %hash,
            peer = %self.node_id,
            "AssetReady received - downloading blob"
        );

        // 1. Download blob from peer (real transport or mock)
        let plaintext = match &state.blob_store {
            BlobStore::Real(transport) => {
                let iroh_hash = transport::BlobHash::from_bytes(*iroh_hash_bytes);
                match transport.download_blob(iroh_hash, self.node_id).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!(hash = %hash, error = ?e, "Failed to download blob from peer");
                        self.send_message(&Message::AssetAck {
                            page_id: page_id.to_string(),
                            hash: hash.to_string(),
                            success: false,
                            error: Some(format!("Download failed: {}", e)),
                        }, state).await;
                        return;
                    }
                }
            }
            BlobStore::Mock(mock_store) => {
                match mock_store.download_blob(iroh_hash_bytes) {
                    Some(data) => data,
                    None => {
                        warn!(hash = %hash, "Blob not found in mock store");
                        self.send_message(&Message::AssetAck {
                            page_id: page_id.to_string(),
                            hash: hash.to_string(),
                            success: false,
                            error: Some("Blob not found in mock store".to_string()),
                        }, state).await;
                        return;
                    }
                }
            }
        };

        // 2. Get page key for storage
        let (_, page_key) = match state.butler.get_decrypted_page(page_id).await {
            Ok(result) => result,
            Err(e) => {
                warn!(page_id = %page_id, error = ?e, "Failed to get page key for storing asset");
                self.send_message(&Message::AssetAck {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                    success: false,
                    error: Some(format!("Page not found: {}", e)),
                }, state).await;
                return;
            }
        };

        // 3. Get metadata from Loro-synced assets layer (for signature verification)
        let assets_layer_name = format!("{}/assets", page_id);
        let metadata = match self.get_asset_metadata_from_layer(page_id, &assets_layer_name, hash, state).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                warn!(hash = %hash, "Asset metadata not found in layer - cannot verify");
                self.send_message(&Message::AssetAck {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                    success: false,
                    error: Some("Metadata not synced yet".to_string()),
                }, state).await;
                return;
            }
            Err(e) => {
                warn!(hash = %hash, error = %e, "Failed to get asset metadata from layer");
                self.send_message(&Message::AssetAck {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                    success: false,
                    error: Some(format!("Metadata lookup failed: {}", e)),
                }, state).await;
                return;
            }
        };

        // 4. Verify and store the received asset
        if let Err(e) = butler::services::asset_service::store_received(
            state.butler.asset_store(),
            &page_key,
            &metadata,
            &plaintext,
        ) {
            warn!(hash = %hash, error = ?e, "Failed to store received asset");
            self.send_message(&Message::AssetAck {
                page_id: page_id.to_string(),
                hash: hash.to_string(),
                success: false,
                error: Some(format!("Storage failed: {}", e)),
            }, state).await;
            return;
        }

        info!(
            page_id = %page_id,
            hash = %hash,
            size = plaintext.len(),
            "Asset received and stored successfully"
        );

        // 5. Send AssetAck
        self.send_message(&Message::AssetAck {
            page_id: page_id.to_string(),
            hash: hash.to_string(),
            success: true,
            error: None,
        }, state).await;
    }

    /// Handle AssetAck from peer
    ///
    /// **Context**: Peer received and processed our AssetReady message
    /// **Success**: Remove from pending transfers, log completion
    /// **Failure**: Retry with exponential backoff (max 3 attempts)
    async fn on_asset_ack(
        &self,
        page_id: &str,
        hash: &str,
        success: bool,
        error: Option<&str>,
        state: &mut PeerActorState,
    ) {
        if success {
            // Success - remove from pending transfers
            state.pending_asset_transfers.remove(hash);
            info!(
                page_id = %page_id,
                hash = %hash,
                peer = %self.node_id,
                "Asset transfer acknowledged"
            );
            // MemStore auto-cleans via reference counting, no manual cleanup needed
        } else {
            // Failure - check retry logic
            let should_retry = if let Some(pending) = state.pending_asset_transfers.get_mut(hash) {
                pending.attempts += 1;
                if pending.attempts < MAX_ASSET_TRANSFER_ATTEMPTS {
                    let backoff_ms = 100 * (1 << pending.attempts); // Exponential: 200ms, 400ms, 800ms
                    info!(
                        page_id = %page_id,
                        hash = %hash,
                        attempt = pending.attempts,
                        backoff_ms = backoff_ms,
                        error = ?error,
                        "Asset transfer failed, scheduling retry"
                    );
                    // Return true to retry
                    true
                } else {
                    warn!(
                        page_id = %page_id,
                        hash = %hash,
                        attempts = pending.attempts,
                        error = ?error,
                        "Asset transfer failed after max retries, giving up (will sync on next session)"
                    );
                    // Give up - remove from pending
                    false
                }
            } else {
                // Not tracked - this is a response to an externally triggered AssetPrepare
                // or the transfer completed before we tracked it
                warn!(
                    page_id = %page_id,
                    hash = %hash,
                    error = ?error,
                    "Asset transfer failed (not tracked for retry)"
                );
                false
            };

            if should_retry {
                // Re-send AssetPrepare for retry
                self.send_message(&Message::AssetPrepare {
                    page_id: page_id.to_string(),
                    hash: hash.to_string(),
                }, state).await;
            } else {
                // Clean up
                state.pending_asset_transfers.remove(hash);
            }
        }
    }

    /// Get asset metadata from the Loro-synced assets layer
    ///
    /// **Context**: When receiving an asset via iroh-blobs, we need the full metadata
    /// (including signature) from the Loro layer for verification
    ///
    /// **Flow**:
    ///   1. Get Scribe for the page
    ///   2. Get layer snapshot
    ///   3. Parse and find the specific asset metadata by hash
    async fn get_asset_metadata_from_layer(
        &self,
        page_id: &str,
        layer_name: &str,
        hash: &str,
        state: &mut PeerActorState,
    ) -> Result<Option<butler::AssetMetadata>, String> {
        // Get Scribe
        let scribe = match state.page_subscriptions.get(page_id) {
            Some(sub) => sub.scribe.clone(),
            None => {
                match state.butler.open_page(page_id).await {
                    Ok(s) => s,
                    Err(e) => return Err(format!("Failed to open page: {}", e)),
                }
            }
        };

        // Get layer snapshot
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(e) = scribe.cast(butler::ScribeMessage::GetSnapshot {
            layer_name: layer_name.to_string(),
            reply: tx,
        }) {
            return Err(format!("Failed to request layer snapshot: {}", e));
        }

        let layer_bytes = match rx.await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Ok(None), // Layer doesn't exist
            Err(_) => return Err("Scribe dropped reply channel".to_string()),
        };

        // Parse layer
        let layer = butler::models::Layer::from_snapshot(&layer_bytes)
            .map_err(|e| format!("Failed to parse layer: {}", e))?;

        // Get all assets from layer and find the one we need
        let assets = butler::services::asset_service::get_assets_from_layer(&layer);
        Ok(assets.get(hash).cloned())
    }

    /// Handle ephemeral datagram received from peer
    ///
    /// **Context**: Datagram received via PeerActor's read loop
    /// **We do**: Deserialize, route to Scribe via existing page_subscriptions
    /// **Scribe then**: Emits to local app AND relays to other subscribers (node mode)
    async fn handle_datagram(&self, data: &[u8], state: &PeerActorState) {
        // Deserialize the datagram
        let datagram: EphemeralDatagram = match EphemeralDatagram::from_bytes(data) {
            Ok(d) => d,
            Err(e) => {
                debug!("Failed to deserialize ephemeral datagram from {}: {}", self.node_id, e);
                return;
            }
        };

        // Route to Scribe via existing page_subscriptions
        if let Some(subscription) = state.page_subscriptions.get(&datagram.page_id) {
            // Get peer's DID from state (set during handshake)
            let user_did = state.state.did().map(|s| s.to_string());
            // Use node_id as device_id for sender exclusion during relay
            let device_id = Some(self.node_id.to_string());

            // Send to Scribe - it will forward to app layer AND relay to other peers
            if let Err(e) = subscription.scribe.cast(ScribeMessage::RemoteEphemeral {
                user_did,
                device_id,
                payload: datagram.payload,
            }) {
                debug!("Failed to forward ephemeral to Scribe for page {}: {}", datagram.page_id, e);
            }
        } else {
            debug!(
                "No subscription for page {} - dropping ephemeral from {}",
                datagram.page_id, self.node_id
            );
        }
    }
}
